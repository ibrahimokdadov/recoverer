// engine/src/main.rs
mod commands;
mod error;
mod events;
mod filetype;
mod pipe;
mod recovery;
mod scan;
mod sessions;
mod store;

use commands::{Command, ScanDepth};
use events::{Event, RecoveryStatus};
use scan::orchestrator::{ScanConfig, ScanOrchestrator};
use sessions::{SessionsStore, new_session_db_path};
use store::Store;
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    // Log to %APPDATA%\Recoverer\engine.log so we can diagnose issues without a terminal
    let log_path = {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        let dir = std::path::Path::new(&appdata).join("Recoverer");
        std::fs::create_dir_all(&dir).ok();
        dir.join("engine.log")
    };
    if let Ok(file) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
            .target(env_logger::Target::Pipe(Box::new(file)))
            .init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    }
    log::info!("Recoverer engine v{}", env!("CARGO_PKG_VERSION"));

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<Command>(64);
    let (event_tx, event_rx) = mpsc::channel::<Event>(1024);

    // Start the named pipe server (handles UI connection)
    let pipe_event_tx = event_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = pipe::run_pipe_server(cmd_tx, event_rx).await {
            log::error!("Pipe server error: {}", e);
        }
    });

    // Paths
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let recoverer_dir = std::path::Path::new(&appdata).join("Recoverer");
    std::fs::create_dir_all(&recoverer_dir).ok();
    let sessions_dir = recoverer_dir.join("sessions");
    std::fs::create_dir_all(&sessions_dir).ok();
    let sessions_meta = recoverer_dir.join("sessions.db").to_string_lossy().to_string();

    // Sessions index
    let sessions_store = match SessionsStore::open(&sessions_meta) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to open sessions index: {}", e);
            // Fall back to legacy path
            let legacy = recoverer_dir.join("scan_results.db").to_string_lossy().to_string();
            SessionsStore::open(&legacy).expect("Cannot open sessions store")
        }
    };

    // Active DB: start with legacy scan_results.db for backward compat
    // (if it has files it'll show as unnamed; new scans get timestamped DBs)
    let mut active_db_path = recoverer_dir.join("scan_results.db").to_string_lossy().to_string();

    // State
    let mut active_cancel: Option<Arc<std::sync::atomic::AtomicBool>> = None;
    let mut active_pause:  Option<Arc<std::sync::atomic::AtomicBool>> = None;

    // Command dispatch loop
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            Command::Ping => {
                let _ = pipe_event_tx.send(Event::Pong).await;
            }

            Command::StartScan { drive, depth, categories } => {
                // Cancel any active scan
                if let Some(cancel) = &active_cancel {
                    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }

                // Create a new session DB (each scan gets its own file)
                let session_db = new_session_db_path(&sessions_dir, &drive);
                let now_ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                let session_name = format!("{} drive — scan {}", drive, now_ts);
                sessions_store.register(&session_name, &drive, &session_db, now_ts).ok();
                active_db_path = session_db.clone();
                log::info!("New session DB: {}", session_db);

                let store = match Store::open(&active_db_path) {
                    Ok(s) => Arc::new(s),
                    Err(e) => {
                        let _ = pipe_event_tx.send(Event::Error {
                            code: "DB_ERROR".to_string(),
                            message: e.to_string(),
                            fatal: true,
                        }).await;
                        continue;
                    }
                };

                // Save drive for recovery phase
                store.save_checkpoint("scan_drive", &drive).ok();
                // Reset MFT checkpoint so scan starts fresh
                let ck = format!("mft_record_idx_{}", drive.trim_end_matches(':'));
                store.save_checkpoint(&ck, "0").ok();

                let config = ScanConfig {
                    drive,
                    db_path: active_db_path.clone(),
                    categories,
                    deep_scan: depth == ScanDepth::Deep,
                    carve_only: depth == ScanDepth::CarveOnly,
                };

                let orchestrator = ScanOrchestrator::new(config, store, pipe_event_tx.clone());
                let cancel = orchestrator.cancel_handle();
                let pause  = orchestrator.pause_handle();
                active_cancel = Some(cancel);
                active_pause  = Some(pause);

                let etx = pipe_event_tx.clone();
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("Failed to build scan runtime");
                    rt.block_on(async move {
                        if let Err(e) = orchestrator.run().await {
                            let _ = etx.send(Event::Error {
                                code: "SCAN_ERROR".to_string(),
                                message: e.to_string(),
                                fatal: false,
                            }).await;
                        }
                    });
                });
            }

            Command::PauseScan => {
                if let Some(pause) = &active_pause {
                    pause.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }

            Command::ResumeScan => {
                if let Some(pause) = &active_pause {
                    pause.store(false, std::sync::atomic::Ordering::Relaxed);
                }
            }

            Command::CancelScan => {
                if let Some(cancel) = &active_cancel {
                    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }

            Command::ListSessions => {
                match sessions_store.list() {
                    Ok(sessions) => {
                        let _ = pipe_event_tx.send(Event::SessionsList { sessions }).await;
                    }
                    Err(e) => log::warn!("ListSessions failed: {}", e),
                }
            }

            Command::ApplyScanHistory => {
                // 1. Open active session DB
                let store = match Store::open(&active_db_path) {
                    Ok(s) => s,
                    Err(e) => {
                        log::warn!("ApplyScanHistory: can't open DB: {}", e);
                        let _ = pipe_event_tx.send(Event::Pong).await;
                        continue;
                    }
                };
                // 2. Persist any newly recovered clusters from this session to the shared index
                let drive = store.load_checkpoint("scan_drive")
                    .ok().flatten().unwrap_or_default();
                if !drive.is_empty() {
                    if let Ok(new_clusters) = store.get_recovered_clusters() {
                        if !new_clusters.is_empty() {
                            sessions_store.record_recovered_clusters(&drive, &new_clusters).ok();
                            log::info!("ApplyScanHistory: persisted {} recovered clusters for drive {}", new_clusters.len(), drive);
                        }
                    }
                    // 3. Load the full history for this drive and mark matching files
                    if let Ok(all_clusters) = sessions_store.get_recovered_clusters(&drive) {
                        if !all_clusters.is_empty() {
                            match store.bulk_mark_recovered_by_clusters(&all_clusters) {
                                Ok(n) => log::info!("ApplyScanHistory: marked {} files as recovered from history (drive {})", n, drive),
                                Err(e) => log::warn!("ApplyScanHistory: bulk mark failed: {}", e),
                            }
                        }
                    }
                }
                let _ = pipe_event_tx.send(Event::Pong).await;
            }

            Command::SwitchSession { session_id } => {
                match sessions_store.get_db_path_by_id(session_id) {
                    Ok(Some(path)) => {
                        log::info!("SwitchSession → {}", path);
                        active_db_path = path;
                        // Ack so UI knows switch is complete before it sends QueryFiles
                        let _ = pipe_event_tx.send(Event::Pong).await;
                    }
                    Ok(None) => log::warn!("SwitchSession: session {} not found", session_id),
                    Err(e)   => log::warn!("SwitchSession error: {}", e),
                }
            }

            Command::QueryFiles { category, min_confidence, name_contains, offset, limit, exclude_recovered, collapse_fragments } => {
                let store = match Store::open(&active_db_path) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let files = store.query_files(
                    category.as_deref(),
                    min_confidence,
                    name_contains.as_deref(),
                    exclude_recovered,
                    collapse_fragments,
                    offset as i64,
                    limit as i64,
                ).unwrap_or_default();

                let total = store.total_count(
                    category.as_deref(),
                    min_confidence,
                    name_contains.as_deref(),
                    exclude_recovered,
                    collapse_fragments,
                ).unwrap_or(0);

                let _ = pipe_event_tx.send(Event::FilesPage { files, total_count: total }).await;
            }

            Command::RecoverFiles { file_ids, destination, recreate_structure } => {
                let store = match Store::open(&active_db_path) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                let scanned_drive = store.load_checkpoint("scan_drive")
                    .ok().flatten().unwrap_or_default();

                if recovery::is_same_volume(&scanned_drive, &destination) {
                    let _ = pipe_event_tx.send(Event::Error {
                        code: "SAME_VOLUME".to_string(),
                        message: "Destination is on the same volume as the scanned drive. This would overwrite recoverable data.".to_string(),
                        fatal: false,
                    }).await;
                    continue;
                }

                let opts = recovery::RecoveryOptions {
                    destination: destination.clone(),
                    recreate_structure,
                    on_conflict: recovery::ConflictMode::AddSuffix,
                };

                let total = file_ids.len() as u64;
                let etx = pipe_event_tx.clone();
                let db = active_db_path.clone();
                let drive_for_recovery = scanned_drive.clone();

                std::thread::spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all().build().expect("recovery rt");
                    rt.block_on(async move {
                        log::info!("Recovery: opening volume {:?} for {} files -> {:?}", drive_for_recovery, total, opts.destination);
                        let volume_reader = match scan::volume::VolumeReader::open(&drive_for_recovery) {
                            Ok(r) => { log::info!("Recovery: volume opened ok"); Some(r) }
                            Err(e) => { log::error!("Recovery: failed to open volume: {}", e); None }
                        };
                        let spc: u64 = volume_reader.as_ref()
                            .and_then(|r| r.read_sector(0).ok())
                            .and_then(|boot| scan::ntfs::parse_boot_sector(&boot).ok())
                            .map(|b| b.sectors_per_cluster as u64)
                            .unwrap_or(8);

                        let store = match Store::open(&db) {
                            Ok(s) => s,
                            Err(e) => {
                                let _ = etx.send(Event::Error {
                                    code: "DB_ERROR".to_string(),
                                    message: e.to_string(),
                                    fatal: false,
                                }).await;
                                return;
                            }
                        };
                        let mut recovered = 0u64;
                        let mut warnings  = 0u64;
                        let mut failed    = 0u64;
                        let bps = volume_reader.as_ref()
                            .map(|r| r.bytes_per_sector as u64)
                            .unwrap_or(512);

                        // Track which fragment groups we've already recovered
                        let mut recovered_groups = std::collections::HashSet::<i64>::new();

                        for id in &file_ids {
                            let file = match store.get_file_by_id(*id) {
                                Ok(Some(f)) => f,
                                Ok(None) | Err(_) => {
                                    failed += 1;
                                    let _ = etx.send(Event::RecoveryProgress {
                                        recovered, warnings, failed, total,
                                    }).await;
                                    continue;
                                }
                            };

                            // If this file belongs to a fragment group and the group is
                            // already recovered (via another member), skip silently.
                            let group = file.fragment_group_id;
                            if group > 0 && recovered_groups.contains(&group) {
                                continue;
                            }

                            let dst_path = recovery::build_destination_path(
                                &opts,
                                file.filename.as_deref(),
                                file.original_path.as_deref(),
                                &file.mime_type,
                                file.id,
                            );

                            // Build a single (start_sector, total_bytes) span to read.
                            // For fragment chains: read the full contiguous span from the first
                            // fragment's start to the last fragment's end in one pass — avoids
                            // reading N × default_size (which for 1 000 video chunks = 50 TB).
                            // For standalone files: just the file's own sector range.
                            let read_jobs: Vec<(u64, u64)> = if group > 0 {
                                if let Ok(siblings) = store.get_fragment_siblings_with_size(group) {
                                    if siblings.is_empty() { vec![] } else {
                                        let first_sector = siblings[0].1;
                                        let last = siblings.last().unwrap();
                                        let tail_default: u64 =
                                            if file.mime_type.starts_with("video/")      { 200 * 1024 * 1024 }
                                            else if file.mime_type.starts_with("audio/") {  20 * 1024 * 1024 }
                                            else                                          {   1 * 1024 * 1024 };
                                        let last_end = if last.2 > 0 {
                                            last.1 + (last.2 + bps - 1) / bps
                                        } else {
                                            last.1 + (tail_default + bps - 1) / bps
                                        };
                                        let span_bytes = (last_end.saturating_sub(first_sector)) * bps;
                                        if span_bytes > 0 { vec![(first_sector, span_bytes)] } else { vec![] }
                                    }
                                } else { vec![] }
                            } else {
                                let cluster_info = store.get_file_cluster_and_size(file.id).ok().flatten();
                                if let Some((cluster, size, source)) = cluster_info {
                                    let sector = if source == "carved" { cluster } else { cluster * spc };
                                    let read_size = if size > 0 { size }
                                        else if file.mime_type.starts_with("video/") { 200 * 1024 * 1024 }
                                        else if file.mime_type.starts_with("audio/") { 20 * 1024 * 1024 }
                                        else if file.mime_type.starts_with("image/") { 5 * 1024 * 1024 }
                                        else { 1 * 1024 * 1024 };
                                    vec![(sector, read_size)]
                                } else { vec![] }
                            };

                            let written = if let (Some(reader), false) = (&volume_reader, read_jobs.is_empty()) {
                                let final_path = recovery::resolve_conflict(&dst_path, &opts.on_conflict);
                                if let Some(ref path) = final_path {
                                    if let Some(parent) = path.parent() {
                                        std::fs::create_dir_all(parent).ok();
                                    }
                                    let mut ok = true;
                                    let file_handle = std::fs::OpenOptions::new()
                                        .create(true).write(true).truncate(true)
                                        .open(path);
                                    if let Ok(mut fh) = file_handle {
                                        use std::io::Write;
                                        // Read in 32 MB chunks to avoid large allocations for big spans
                                        const CHUNK_BYTES: u64 = 32 * 1024 * 1024;
                                        'jobs: for &(start_sector, total_bytes) in &read_jobs {
                                            let end_sector = start_sector + (total_bytes + bps - 1) / bps;
                                            let mut pos = start_sector;
                                            let mut remaining = total_bytes;
                                            while pos < end_sector && remaining > 0 {
                                                let chunk = remaining.min(CHUNK_BYTES);
                                                let sectors = ((chunk + bps - 1) / bps) as u32;
                                                match reader.read_sectors(pos, sectors) {
                                                    Ok(data) => {
                                                        let trimmed = &data[..data.len().min(chunk as usize)];
                                                        if fh.write_all(trimmed).is_err() { ok = false; break 'jobs; }
                                                    }
                                                    Err(_) => { ok = false; break 'jobs; }
                                                }
                                                pos += sectors as u64;
                                                remaining = remaining.saturating_sub(chunk);
                                            }
                                        }
                                        ok
                                    } else { false }
                                } else { true /* skipped */ }
                            } else { false };

                            if written {
                                // Mark all siblings (or just this file) as recovered
                                if group > 0 {
                                    if let Ok(siblings) = store.get_fragment_siblings(group) {
                                        for (sid, _) in siblings {
                                            store.update_recovery_status(sid, RecoveryStatus::Recovered).ok();
                                        }
                                    }
                                    recovered_groups.insert(group);
                                } else {
                                    store.update_recovery_status(file.id, RecoveryStatus::Recovered).ok();
                                }
                                if file.confidence < 60 { warnings += 1; } else { recovered += 1; }
                            } else {
                                failed += 1;
                            }

                            let _ = etx.send(Event::RecoveryProgress {
                                recovered, warnings, failed, total,
                            }).await;
                        }

                        let _ = etx.send(Event::RecoveryComplete { recovered, warnings, failed }).await;
                    });
                });
            }
        }
    }
}
