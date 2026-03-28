// engine/src/main.rs
mod commands;
mod error;
mod events;
mod filetype;
mod pipe;
mod recovery;
mod scan;
mod store;

use commands::{Command, ScanDepth};
use events::{Event, RecoveryStatus};
use scan::orchestrator::{ScanConfig, ScanOrchestrator};
use store::Store;
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
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

    // State
    let mut active_cancel: Option<Arc<std::sync::atomic::AtomicBool>> = None;
    let mut active_pause: Option<Arc<std::sync::atomic::AtomicBool>> = None;
    let db_path = get_db_path();

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

                let store = match Store::open(&db_path) {
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

                let config = ScanConfig {
                    drive,
                    db_path: db_path.clone(),
                    categories,
                    deep_scan: depth == ScanDepth::Deep,
                };

                let orchestrator = ScanOrchestrator::new(config, store, pipe_event_tx.clone());
                let cancel = orchestrator.cancel_handle();
                let pause = orchestrator.pause_handle();
                active_cancel = Some(cancel);
                active_pause = Some(pause);

                let etx = pipe_event_tx.clone();
                // ScanOrchestrator holds a VolumeReader (non-Send raw HANDLE on Windows).
                // Run it on a dedicated OS thread via spawn_blocking to avoid Send requirement.
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

            Command::QueryFiles { category, min_confidence, name_contains, offset, limit } => {
                let store = match Store::open(&db_path) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let files = store.query_files(
                    category.as_deref(),
                    min_confidence,
                    name_contains.as_deref(),
                    offset as i64,
                    limit as i64,
                ).unwrap_or_default();

                let total = store.total_count(
                    category.as_deref(),
                    min_confidence,
                    name_contains.as_deref(),
                ).unwrap_or(0);

                let _ = pipe_event_tx.send(Event::FilesPage { files, total_count: total }).await;
            }

            Command::RecoverFiles { file_ids, destination, recreate_structure } => {
                let store = match Store::open(&db_path) {
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
                let db = db_path.clone();

                tokio::spawn(async move {
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
                    let mut warnings = 0u64;
                    let mut failed = 0u64;

                    for id in &file_ids {
                        match store.get_file_by_id(*id) {
                            Ok(Some(file)) => {
                                let _dst_path = recovery::build_destination_path(
                                    &opts,
                                    file.filename.as_deref(),
                                    file.original_path.as_deref(),
                                    &file.mime_type,
                                    file.id,
                                );

                                // TODO: actual cluster-based recovery for v1
                                match store.update_recovery_status(file.id, RecoveryStatus::Recovered) {
                                    Ok(_) => {
                                        if file.confidence < 60 { warnings += 1; } else { recovered += 1; }
                                    }
                                    Err(_) => { failed += 1; }
                                }

                                let _ = etx.send(Event::RecoveryProgress {
                                    recovered, warnings, failed, total,
                                }).await;
                            }
                            Ok(None) => {
                                failed += 1;
                                let _ = etx.send(Event::RecoveryProgress {
                                    recovered, warnings, failed, total,
                                }).await;
                            }
                            Err(_) => {
                                failed += 1;
                            }
                        }
                    }

                    let _ = etx.send(Event::RecoveryComplete { recovered, warnings, failed }).await;
                });
            }
        }
    }
}

fn get_db_path() -> String {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let dir = std::path::Path::new(&appdata).join("Recoverer");
    std::fs::create_dir_all(&dir).ok();
    dir.join("scan_results.db").to_string_lossy().to_string()
}
