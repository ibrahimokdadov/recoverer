// engine/src/scan/orchestrator.rs
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}};
use tokio::sync::mpsc;
use rayon::prelude::*;
use crate::error::Result;
use crate::events::Event;
use crate::filetype::{detect_file_type, detect_file_type_from_name};
use crate::scan::{ntfs, volume::VolumeReader};
use crate::scan::carver::carve_buffer;
use crate::store::{Store, NewFile};

pub struct ScanConfig {
    pub drive: String,
    pub db_path: String,
    pub categories: Vec<String>,  // empty = all
    pub deep_scan: bool,
    pub carve_only: bool,         // skip MFT, go straight to raw carving
}

pub struct ScanOrchestrator {
    config: ScanConfig,
    store: Arc<Store>,
    event_tx: mpsc::Sender<Event>,
    paused: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
    files_found: Arc<AtomicU64>,
}

impl ScanOrchestrator {
    pub fn new(
        config: ScanConfig,
        store: Arc<Store>,
        event_tx: mpsc::Sender<Event>,
    ) -> Self {
        Self {
            config,
            store,
            event_tx,
            paused: Arc::new(AtomicBool::new(false)),
            cancelled: Arc::new(AtomicBool::new(false)),
            files_found: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn pause_handle(&self) -> Arc<AtomicBool> { self.paused.clone() }
    pub fn cancel_handle(&self) -> Arc<AtomicBool> { self.cancelled.clone() }

    pub async fn run(self) -> Result<()> {
        let start = std::time::Instant::now();
        let drive = self.config.drive.clone();
        log::info!("Scan started: drive={:?} deep={}", drive, self.config.deep_scan);

        // Phase 1: VSS (stub for v1 — emits nothing, completes immediately)
        self.emit(Event::PhaseChange { new_phase: "vss".to_string() }).await;

        if self.cancelled.load(Ordering::Relaxed) { return Ok(()); }

        // Phase 2: Open volume
        log::info!("Opening volume: {}", drive);
        let reader = match VolumeReader::open(&drive) {
            Ok(r) => { log::info!("Volume opened: bytes_per_sector={} total_sectors={}", r.bytes_per_sector, r.total_sectors); r }
            Err(e) => { log::error!("Failed to open volume {}: {}", drive, e); return Err(e); }
        };

        // Phase 3: MFT scan (skipped in carve_only mode)
        if !self.config.carve_only {
            self.emit(Event::PhaseChange { new_phase: "mft_scan".to_string() }).await;
            self.run_mft_scan(&reader).await?;
            if self.cancelled.load(Ordering::Relaxed) { return Ok(()); }
        }

        // Phase 4: Raw carving (deep scan or carve_only)
        if self.config.deep_scan || self.config.carve_only {
            self.emit(Event::PhaseChange { new_phase: "carving".to_string() }).await;
            self.run_carving(&reader).await?;

            // Phase 4b: Fragment chain detection (post-carving pass)
            self.emit(Event::PhaseChange { new_phase: "fragment_grouping".to_string() }).await;
            self.detect_fragments(reader.bytes_per_sector as u64);
        }

        let total = self.files_found.load(Ordering::Relaxed);
        let duration = start.elapsed().as_secs();
        self.emit(Event::ScanComplete { total_found: total, duration_secs: duration }).await;

        Ok(())
    }

    async fn run_mft_scan(&self, reader: &VolumeReader) -> Result<()> {
        let boot_sector = reader.read_sector(0)?;
        let boot = match ntfs::parse_boot_sector(&boot_sector) {
            Ok(b) => {
                log::info!("NTFS boot: bps={} spc={} total_sectors={} mft_lcn={}",
                    b.bytes_per_sector, b.sectors_per_cluster, b.total_sectors, b.mft_lcn);
                b
            }
            Err(e) => {
                log::warn!("Not NTFS ({}), skipping MFT phase", e);
                return Ok(());
            }
        };

        if boot.mft_lcn <= 0 { return Ok(()); }

        let mft_start_sector = boot.mft_lcn as u64 * boot.sectors_per_cluster as u64;
        let record_size_sectors = (1024u64 / boot.bytes_per_sector as u64).max(1);
        let chunk_records = 256u64;
        let chunk_sectors = chunk_records * record_size_sectors;

        // ── Step 1: read MFT record 0 to discover all MFT extents ──────────
        // The MFT is itself an NTFS file ($MFT, record 0). Its $DATA attribute
        // encodes all physical extents as a run-list.  On fragmented or mature
        // volumes there can be multiple extents; skipping them means missing
        // every deleted file whose MFT slot lives in extent 2+.
        let sector_runs: Vec<(u64, u64)>;
        let total_mft_records: u64;

        match reader.read_sectors(mft_start_sector, record_size_sectors as u32) {
            Ok(raw) => {
                let mut rec0 = raw[..1024.min(raw.len())].to_vec();
                ntfs::apply_fixup(&mut rec0);
                let (runs, total) = ntfs::parse_mft_extents(&rec0, &boot);
                if !runs.is_empty() {
                    sector_runs = runs;
                    total_mft_records = total;
                } else {
                    log::warn!("Could not parse MFT run-list — falling back to linear heuristic");
                    let heuristic = (boot.total_sectors * boot.bytes_per_sector as u64
                        / (10 * 1024 * 1024)).max(1000);
                    sector_runs = vec![(mft_start_sector, heuristic * record_size_sectors)];
                    total_mft_records = heuristic;
                }
            }
            Err(e) => {
                log::warn!("Failed to read MFT record 0: {} — aborting MFT scan", e);
                return Ok(());
            }
        }

        // ── Step 2: build a flat list of chunks across all extents ──────────
        // This lets checkpointing work with a single global record_idx counter.
        struct RunInfo { start_sector: u64, length_sectors: u64, first_record: u64 }
        let mut run_infos: Vec<RunInfo> = Vec::new();
        let mut global = 0u64;
        for (s, l) in &sector_runs {
            let rec_count = l / record_size_sectors;
            run_infos.push(RunInfo { start_sector: *s, length_sectors: *l, first_record: global });
            global += rec_count;
        }

        // ── Step 3: restore checkpoint ──────────────────────────────────────
        let checkpoint_key = format!("mft_record_idx_{}", self.config.drive.trim_end_matches(':'));
        let resume_from: u64 = self.store.load_checkpoint(&checkpoint_key)
            .ok().flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if resume_from > 0 {
            log::info!("Resuming MFT scan from record {}", resume_from);
        }

        // ── Step 4: iterate extents ─────────────────────────────────────────
        'extents: for run in &run_infos {
            let run_record_count = run.length_sectors / record_size_sectors;
            let run_end = run.first_record + run_record_count;

            // Skip extents fully before the checkpoint
            if resume_from >= run_end { continue; }

            // Compute sector offset within this extent for a mid-extent resume
            let skip_records = resume_from.saturating_sub(run.first_record);
            let mut sector_in_run = skip_records * record_size_sectors;

            while sector_in_run < run.length_sectors {
                if self.cancelled.load(Ordering::Relaxed) { break 'extents; }
                while self.paused.load(Ordering::Relaxed) {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }

                let available = run.length_sectors - sector_in_run;
                let read_sectors = (chunk_sectors).min(available) as u32;
                // Align to record boundary
                let read_sectors = (read_sectors / record_size_sectors as u32) * record_size_sectors as u32;
                if read_sectors == 0 { break; }

                let start_sector = run.start_sector + sector_in_run;
                let global_record_at_chunk = run.first_record + sector_in_run / record_size_sectors;

                log::info!("MFT chunk: record_idx={} sector={} pct={}%",
                    global_record_at_chunk, start_sector,
                    (global_record_at_chunk * 100 / total_mft_records.max(1)).min(50));

                let chunk = match reader.read_sectors(start_sector, read_sectors) {
                    Ok(c) => c,
                    Err(e) => {
                        log::warn!("read_sectors failed at sector {}: {}", start_sector, e);
                        sector_in_run += read_sectors as u64;
                        continue;
                    }
                };

                let records_in_chunk = (read_sectors as u64 / record_size_sectors) as usize;
                let parsed: Vec<_> = (0..records_in_chunk)
                    .into_par_iter()
                    .filter_map(|i| {
                        let s = i * 1024;
                        let e = s + 1024;
                        if e > chunk.len() { return None; }
                        ntfs::parse_mft_record(&chunk[s..e],
                            (global_record_at_chunk + i as u64) as u32)
                    })
                    .filter(|r| !r.in_use && !r.is_directory && r.first_data_cluster.is_some())
                    .collect();

                if !parsed.is_empty() {
                    log::info!("MFT chunk {}: {} deleted file(s)", global_record_at_chunk, parsed.len());
                }

                for record in parsed {
                    let ftype = if let Some(ref name) = record.filename {
                        detect_file_type_from_name(name)
                    } else {
                        detect_file_type(&[])
                    };

                    if !self.config.categories.is_empty()
                        && !self.config.categories.contains(&ftype.category) {
                        continue;
                    }

                    // cluster=0 means $ATTRIBUTE_LIST placeholder — skip (can't recover without full run-list)
                    let cluster = record.first_data_cluster.filter(|&c| c > 0);
                    if cluster.is_none() { continue; }

                    let confidence: u8 = if record.filename.as_deref()
                        .map_or(false, |n| n.contains('.')) { 75 } else { 60 };

                    let new_file = NewFile {
                        filename: record.filename.clone(),
                        original_path: None,
                        mime_type: ftype.mime_type.clone(),
                        category: ftype.category.clone(),
                        size_bytes: record.file_size,
                        first_cluster: cluster,
                        confidence,
                        source: "mft".to_string(),
                        mft_record_number: Some(record.record_number as u64),
                        created_at: record.created_at,
                        modified_at: record.modified_at,
                        deleted_at: None,
                    };

                    let id = self.store.insert_file(&new_file)?;
                    self.files_found.fetch_add(1, Ordering::Relaxed);

                    let _ = self.event_tx.try_send(Event::FileFound {
                        id,
                        filename: record.filename,
                        original_path: None,
                        size_bytes: record.file_size,
                        mime_type: ftype.mime_type,
                        category: ftype.category,
                        confidence,
                        source: "mft".to_string(),
                    });
                }

                sector_in_run += read_sectors as u64;
                let new_checkpoint = run.first_record + sector_in_run / record_size_sectors;
                self.store.save_checkpoint(&checkpoint_key, &new_checkpoint.to_string())?;

                let pct = (new_checkpoint * 100 / total_mft_records.max(1)) as u8;
                let _ = self.event_tx.try_send(Event::Progress {
                    phase: "mft_scan".to_string(),
                    pct: pct.min(50),
                    files_found: self.files_found.load(Ordering::Relaxed),
                    eta_secs: None,
                });
            }
        }

        log::info!("MFT scan complete. Total files found: {}",
            self.files_found.load(Ordering::Relaxed));
        Ok(())
    }

    async fn run_carving(&self, reader: &VolumeReader) -> Result<()> {
        let total_sectors = reader.total_sectors;
        let bps = reader.bytes_per_sector as u64;
        // 8 MB chunks — larger reads amortise USB overhead better than 1 MB
        let chunk_sectors = (8 * 1024 * 1024u64 / bps) as u32;
        let mut sector = 0u64;
        let start_time = std::time::Instant::now();
        // Log every ~1 GB
        let log_interval_sectors = 1024 * 1024 * 1024 / bps;
        let mut next_log_sector = log_interval_sectors;
        let mut carved_this_session = 0u64;

        // Per-MIME dedup: skip carved hits whose start sector falls inside a range
        // already attributed to that MIME type. Reduces thousands of redundant DB entries
        // for large files (e.g. MP4 box headers found throughout a 2 GB video).
        // Key = mime_type, Value = end_sector of last attributed file of that type.
        let mut skip_to: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

        log::info!("Carving: starting raw scan of {} sectors ({:.1} GB) at {} bytes/sector",
            total_sectors,
            total_sectors as f64 * bps as f64 / (1024.0 * 1024.0 * 1024.0),
            bps);

        loop {
            if self.cancelled.load(Ordering::Relaxed) { break; }
            while self.paused.load(Ordering::Relaxed) {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }

            let remaining = total_sectors.saturating_sub(sector);
            if remaining == 0 { break; }
            let read_count = chunk_sectors.min(remaining as u32);

            let buf = match reader.read_sectors(sector, read_count) {
                Ok(b) => b,
                Err(e) => {
                    log::warn!("Carving: read error at sector {} — skipping chunk ({})", sector, e);
                    sector += read_count as u64;
                    continue;
                }
            };

            let base_offset = sector * bps;
            let carved = carve_buffer(&buf, base_offset);

            if !carved.is_empty() {
                log::info!("Carving: sector {} — found {} file header(s): {}",
                    sector,
                    carved.len(),
                    carved.iter().map(|c| c.mime_type.as_str()).collect::<Vec<_>>().join(", "));
            }

            for result in carved {
                if !self.config.categories.is_empty()
                    && !self.config.categories.contains(&result.category) {
                    continue;
                }

                // Skip hits that fall inside a range we already attributed to this MIME type.
                // This prevents thousands of DB entries for box headers within a single large video.
                let hit_sector = result.byte_offset / bps;
                if let Some(&end) = skip_to.get(&result.mime_type) {
                    if hit_sector < end {
                        // Still within a previously found file — update end if this hit extends it
                        if let Some(size) = result.estimated_size {
                            let this_end = hit_sector + (size + bps - 1) / bps;
                            if this_end > end {
                                skip_to.insert(result.mime_type.clone(), this_end);
                            }
                        }
                        continue; // skip — not a new file
                    }
                }

                // New file — record its attributed sector range
                {
                    let end = if let Some(size) = result.estimated_size {
                        hit_sector + (size + bps - 1) / bps
                    } else {
                        // No footer: estimate range using type-specific default
                        let default_bytes: u64 = if result.mime_type.starts_with("video/")      { 100 * 1024 * 1024 }
                                                  else if result.mime_type.starts_with("audio/") {  20 * 1024 * 1024 }
                                                  else                                           {   1 * 1024 * 1024 };
                        hit_sector + (default_bytes + bps - 1) / bps
                    };
                    skip_to.insert(result.mime_type.clone(), end);
                }

                // Footer confirmed → we know the file boundary → higher confidence
                let confidence: u8 = if result.estimated_size.is_some() { 65 } else { 45 };

                let new_file = NewFile {
                    filename: None,
                    original_path: None,
                    mime_type: result.mime_type.clone(),
                    category: result.category.clone(),
                    size_bytes: result.estimated_size.unwrap_or(0),
                    first_cluster: Some(result.byte_offset / bps),
                    confidence,
                    source: "carved".to_string(),
                    mft_record_number: None,
                    created_at: None, modified_at: None, deleted_at: None,
                };

                let id = self.store.insert_file(&new_file)?;
                self.files_found.fetch_add(1, Ordering::Relaxed);
                carved_this_session += 1;

                let _ = self.event_tx.try_send(Event::FileFound {
                    id,
                    filename: None,
                    original_path: None,
                    size_bytes: result.estimated_size.unwrap_or(0),
                    mime_type: result.mime_type,
                    category: result.category,
                    confidence,
                    source: "carved".to_string(),
                });
            }

            sector += read_count as u64;

            // Periodic verbose progress log
            if sector >= next_log_sector {
                let elapsed = start_time.elapsed().as_secs_f64();
                let rate_mbs = (sector as f64 * bps as f64) / elapsed / (1024.0 * 1024.0);
                let remaining_bytes = (total_sectors - sector) as f64 * bps as f64;
                let eta_secs = if rate_mbs > 0.0 {
                    (remaining_bytes / (rate_mbs * 1024.0 * 1024.0)) as u64
                } else { 0 };
                let pct = (sector * 100 / total_sectors.max(1)).min(99);
                log::info!(
                    "Carving: {:.1} GB / {:.1} GB  ({}%)  {:.1} MB/s  ETA {}min  files_found={}",
                    sector as f64 * bps as f64 / (1024.0 * 1024.0 * 1024.0),
                    total_sectors as f64 * bps as f64 / (1024.0 * 1024.0 * 1024.0),
                    pct, rate_mbs, eta_secs / 60,
                    self.files_found.load(Ordering::Relaxed));
                next_log_sector += log_interval_sectors;
            }

            // Progress event for UI
            let elapsed = start_time.elapsed().as_secs_f64();
            let rate_sectors = if elapsed > 0.0 { sector as f64 / elapsed } else { 1.0 };
            let eta_secs = ((total_sectors - sector.min(total_sectors)) as f64 / rate_sectors) as u64;
            let pct = if self.config.carve_only {
                (sector * 99 / total_sectors.max(1)) as u8
            } else {
                (sector * 49 / total_sectors.max(1) + 50) as u8
            };
            let _ = self.event_tx.try_send(Event::Progress {
                phase: "carving".to_string(),
                pct: pct.min(99),
                files_found: self.files_found.load(Ordering::Relaxed),
                eta_secs: Some(eta_secs),
            });
        }

        log::info!("Carving complete. Found {} file(s) this session. Total: {}",
            carved_this_session,
            self.files_found.load(Ordering::Relaxed));
        Ok(())
    }

    /// Post-carving pass: group consecutive carved files of the same MIME type that are
    /// likely chunks of the same large file. Uses per-type gap thresholds:
    ///   video/* → 100 000 sectors (~50 MB) — MP4 box headers recur throughout a file
    ///   audio/* →  50 000 sectors (~25 MB)
    ///   other   →     256 sectors (~128 KB)
    /// After grouping, updates the chain lead's size_bytes to the total span size so the
    /// UI shows the real estimated file size instead of the tiny first-chunk size.
    fn detect_fragments(&self, bps: u64) {
        let candidates = match self.store.get_carved_for_fragment_detection() {
            Ok(c) => c,
            Err(e) => { log::warn!("Fragment detection failed: {}", e); return; }
        };

        if candidates.len() < 2 { return; }

        fn gap_threshold(mime: &str) -> u64 {
            if      mime.starts_with("video/") { 100_000 }
            else if mime.starts_with("audio/") {  50_000 }
            else                               {     256 }
        }

        fn tail_default_sectors(mime: &str, bps: u64) -> u64 {
            let bytes: u64 = if mime.starts_with("video/")      { 50 * 1024 * 1024 }
                             else if mime.starts_with("audio/") { 10 * 1024 * 1024 }
                             else                                {  1 * 1024 * 1024 };
            (bytes + bps - 1) / bps
        }

        let mut group_id: i64 = 1;
        let mut chain: Vec<i64> = Vec::new();
        // (start_sector, size_bytes) per chain member for span computation
        let mut chain_spans: Vec<(u64, u64)> = Vec::new();
        let mut chain_end: u64 = 0;
        let mut chain_mime = String::new();

        let flush_chain = |chain: &[i64], chain_spans: &[(u64, u64)], gid: i64, mime: &str| {
            if chain.len() < 2 { return; }
            if let Err(e) = self.store.set_fragment_group(chain, gid) {
                log::warn!("set_fragment_group failed: {}", e);
                return;
            }
            // Compute span: from first start to last end, use default for unknown tail size
            let first_start = chain_spans[0].0;
            let last = chain_spans.last().unwrap();
            let last_end = if last.1 > 0 {
                last.0 + (last.1 + bps - 1) / bps
            } else {
                last.0 + tail_default_sectors(mime, bps)
            };
            let span_bytes = (last_end.saturating_sub(first_start)) * bps;
            if span_bytes > 0 {
                self.store.update_chain_lead_size(gid, span_bytes).ok();
            }
        };

        for &(id, start_sector, size_bytes, ref _category, ref mime) in &candidates {
            let end_sector = if size_bytes > 0 {
                start_sector + (size_bytes + bps - 1) / bps
            } else {
                start_sector + 1
            };

            if chain.is_empty() {
                chain.push(id);
                chain_spans.push((start_sector, size_bytes));
                chain_end  = end_sector;
                chain_mime = mime.clone();
                continue;
            }

            let gap      = start_sector.saturating_sub(chain_end);
            let max_gap  = gap_threshold(mime);
            let same_mime = *mime == chain_mime;

            if same_mime && gap <= max_gap {
                chain.push(id);
                chain_spans.push((start_sector, size_bytes));
                chain_end = chain_end.max(end_sector);
            } else {
                flush_chain(&chain, &chain_spans, group_id, &chain_mime);
                if chain.len() > 1 { group_id += 1; }
                chain       = vec![id];
                chain_spans = vec![(start_sector, size_bytes)];
                chain_end   = end_sector;
                chain_mime  = mime.clone();
            }
        }
        flush_chain(&chain, &chain_spans, group_id, &chain_mime);
        if chain.len() > 1 { group_id += 1; }

        log::info!("Fragment grouping: {} chain(s) across {} carved files",
            group_id - 1, candidates.len());
    }

    async fn emit(&self, event: Event) {
        let _ = self.event_tx.send(event).await;
    }
}
