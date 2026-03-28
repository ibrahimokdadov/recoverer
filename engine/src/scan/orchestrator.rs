// engine/src/scan/orchestrator.rs
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}};
use tokio::sync::mpsc;
use rayon::prelude::*;
use crate::error::Result;
use crate::events::Event;
use crate::filetype::detect_file_type;
use crate::scan::{ntfs, volume::VolumeReader};
use crate::scan::carver::carve_buffer;
use crate::store::{Store, NewFile};

pub struct ScanConfig {
    pub drive: String,
    pub db_path: String,
    pub categories: Vec<String>,  // empty = all
    pub deep_scan: bool,
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

        // Phase 1: VSS (stub for v1 — emits nothing, completes immediately)
        self.emit(Event::PhaseChange { new_phase: "vss".to_string() }).await;

        if self.cancelled.load(Ordering::Relaxed) { return Ok(()); }

        // Phase 2: Open volume
        let reader = VolumeReader::open(&drive)?;

        // Phase 3: MFT scan
        self.emit(Event::PhaseChange { new_phase: "mft_scan".to_string() }).await;
        self.run_mft_scan(&reader).await?;

        if self.cancelled.load(Ordering::Relaxed) { return Ok(()); }

        // Phase 4: Raw carving (deep scan only)
        if self.config.deep_scan {
            self.emit(Event::PhaseChange { new_phase: "carving".to_string() }).await;
            self.run_carving(&reader).await?;
        }

        let total = self.files_found.load(Ordering::Relaxed);
        let duration = start.elapsed().as_secs();
        self.emit(Event::ScanComplete { total_found: total, duration_secs: duration }).await;

        Ok(())
    }

    async fn run_mft_scan(&self, reader: &VolumeReader) -> Result<()> {
        let boot_sector = reader.read_sector(0)?;
        let boot = match ntfs::parse_boot_sector(&boot_sector) {
            Ok(b) => b,
            Err(_) => {
                // Not NTFS — skip MFT phase
                return Ok(());
            }
        };

        if boot.mft_lcn <= 0 {
            return Ok(());
        }
        let mft_byte_offset = boot.mft_lcn as u64 * boot.bytes_per_cluster as u64;
        let mft_start_sector = mft_byte_offset / boot.bytes_per_sector as u64;
        let record_size_sectors = (1024u64).div_ceil(boot.bytes_per_sector as u64).max(1);

        // Read MFT in chunks of 256 records
        let chunk_records = 256u64;
        let chunk_sectors = chunk_records * record_size_sectors;
        let total_mft_records = boot.total_sectors / 100; // Rough estimate: MFT ~1% of volume

        let mut record_idx = 0u64;

        // Resume from checkpoint if available
        if let Ok(Some(checkpoint)) = self.store.load_checkpoint("mft_record_idx") {
            if let Ok(idx) = checkpoint.parse::<u64>() {
                record_idx = idx;
            }
        }

        loop {
            if self.cancelled.load(Ordering::Relaxed) { break; }

            // Pause support
            while self.paused.load(Ordering::Relaxed) {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }

            let start_sector = mft_start_sector + record_idx * record_size_sectors;
            let chunk = match reader.read_sectors(start_sector, chunk_sectors as u32) {
                Ok(c) => c,
                Err(_) => break,
            };

            // Process records in this chunk (CPU-bound, use rayon in a blocking task)
            let records_data = chunk;
            let record_size = 1024usize;

            let parsed: Vec<_> = (0..chunk_records as usize)
                .into_par_iter()
                .filter_map(|i| {
                    let start = i * record_size;
                    let end = start + record_size;
                    if end > records_data.len() { return None; }
                    ntfs::parse_mft_record(&records_data[start..end], (record_idx + i as u64) as u32)
                })
                .filter(|r| !r.in_use && !r.is_directory && r.file_size > 0)
                .collect();

            for record in parsed {
                // Type detection: read first 512 bytes of file data if available
                let type_bytes = if let Some(cluster) = record.first_data_cluster {
                    let sector = cluster * boot.sectors_per_cluster as u64;
                    reader.read_sector(sector).unwrap_or_default()
                } else {
                    vec![]
                };

                let ftype = detect_file_type(&type_bytes);

                // Category filter
                if !self.config.categories.is_empty()
                    && !self.config.categories.contains(&ftype.category) {
                    continue;
                }

                let confidence: u8 = if !type_bytes.is_empty() { 87 } else { 60 };
                let new_file = NewFile {
                    filename: record.filename.clone(),
                    original_path: None, // TODO: resolve parent path from MFT
                    mime_type: ftype.mime_type.clone(),
                    category: ftype.category.clone(),
                    size_bytes: record.file_size,
                    first_cluster: record.first_data_cluster,
                    confidence,
                    source: "mft".to_string(),
                    mft_record_number: Some(record.record_number as u64),
                    created_at: record.created_at,
                    modified_at: record.modified_at,
                    deleted_at: None,
                };

                let id = self.store.insert_file(&new_file)?;
                let files_count = self.files_found.fetch_add(1, Ordering::Relaxed) + 1;

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

                if files_count % 100 == 0 {
                    let pct = ((record_idx * 100) / total_mft_records.max(1)) as u8;
                    let _ = self.event_tx.try_send(Event::Progress {
                        phase: "mft_scan".to_string(),
                        pct: pct.min(50), // MFT scan = first 50% of total progress
                        files_found: files_count,
                        eta_secs: None,
                    });
                }
            }

            record_idx += chunk_records;
            self.store.save_checkpoint("mft_record_idx", &record_idx.to_string())?;

            if record_idx >= total_mft_records {
                break;
            }
        }

        Ok(())
    }

    async fn run_carving(&self, reader: &VolumeReader) -> Result<()> {
        let total_sectors = reader.total_sectors;
        let chunk_sectors = 2048u32; // 1MB per chunk at 512B/sector
        let mut sector = 0u64;

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
                Err(_) => { sector += read_count as u64; continue; }
            };

            let base_offset = sector * reader.bytes_per_sector as u64;
            let carved = carve_buffer(&buf, base_offset);

            for result in carved {
                if !self.config.categories.is_empty()
                    && !self.config.categories.contains(&result.category) {
                    continue;
                }

                let new_file = NewFile {
                    filename: None,
                    original_path: None,
                    mime_type: result.mime_type.clone(),
                    category: result.category.clone(),
                    size_bytes: result.estimated_size.unwrap_or(0),
                    first_cluster: Some(sector),
                    confidence: 45,
                    source: "carved".to_string(),
                    mft_record_number: None,
                    created_at: None, modified_at: None, deleted_at: None,
                };

                let id = self.store.insert_file(&new_file)?;
                let files_count = self.files_found.fetch_add(1, Ordering::Relaxed) + 1;

                let _ = self.event_tx.try_send(Event::FileFound {
                    id,
                    filename: None,
                    original_path: None,
                    size_bytes: result.estimated_size.unwrap_or(0),
                    mime_type: result.mime_type,
                    category: result.category,
                    confidence: 45,
                    source: "carved".to_string(),
                });

                if files_count % 200 == 0 {
                    let pct = (sector * 50 / total_sectors.max(1) + 50) as u8;
                    let _ = self.event_tx.try_send(Event::Progress {
                        phase: "carving".to_string(),
                        pct: pct.min(99),
                        files_found: files_count,
                        eta_secs: None,
                    });
                }
            }

            sector += read_count as u64;
        }

        Ok(())
    }

    async fn emit(&self, event: Event) {
        let _ = self.event_tx.send(event).await;
    }
}
