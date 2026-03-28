// engine/src/pipe.rs
/// Named pipe server for IPC between UI shell and Rust engine.
/// Protocol: newline-delimited JSON.
/// Pipe name: \\.\pipe\recoverer-engine
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

#[cfg(target_os = "windows")]
use tokio::net::windows::named_pipe::ServerOptions;

use crate::commands::Command;
use crate::events::Event;
use crate::error::Result;

pub const PIPE_NAME: &str = r"\\.\pipe\recoverer-engine";

/// Run the named pipe server. Accepts one client at a time.
/// Reads commands from the pipe, sends responses via `event_rx`.
#[cfg(target_os = "windows")]
pub async fn run_pipe_server(
    cmd_tx: mpsc::Sender<Command>,
    mut event_rx: mpsc::Receiver<Event>,
) -> Result<()> {
    loop {
        let server = ServerOptions::new()
            .first_pipe_instance(false)
            .create(PIPE_NAME)
            .map_err(crate::error::EngineError::Io)?;

        log::info!("Waiting for UI connection on {}", PIPE_NAME);
        server.connect().await
            .map_err(crate::error::EngineError::Io)?;
        log::info!("UI connected");

        let (reader, mut writer) = tokio::io::split(server);
        let mut lines = BufReader::new(reader).lines();

        // Channel to send event_rx back after the write task finishes
        let (return_tx, mut return_rx) = mpsc::channel::<mpsc::Receiver<Event>>(1);

        // Spawn event writer task; moves event_rx in, returns it via return_tx when done
        let write_task = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                match serde_json::to_string(&event) {
                    Ok(mut json) => {
                        json.push('\n');
                        if writer.write_all(json.as_bytes()).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => log::warn!("Event serialization failed: {}", e),
                }
            }
            // Return event_rx to the outer loop
            let _ = return_tx.send(event_rx).await;
        });

        // Read commands
        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim().to_string();
            if line.is_empty() { continue; }
            match serde_json::from_str::<Command>(&line) {
                Ok(cmd) => {
                    if cmd_tx.send(cmd).await.is_err() {
                        break;
                    }
                }
                Err(e) => log::warn!("Unknown command: {} — {}", line, e),
            }
        }

        write_task.abort();
        log::info!("UI disconnected, waiting for next connection");

        // Reclaim event_rx for the next iteration, draining any buffered events
        if let Some(rx) = return_rx.recv().await {
            event_rx = rx;
        } else {
            // write_task was aborted before returning; create a fresh drain channel
            // This means events may be lost for the brief reconnect window — acceptable.
            // We need a new receiver; since we can't recreate the original, break the loop.
            // In practice the abort races with return_tx.send, so try again briefly.
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            if let Some(rx) = return_rx.recv().await {
                event_rx = rx;
            } else {
                log::warn!("Could not reclaim event_rx after client disconnect; stopping pipe server");
                break;
            }
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub async fn run_pipe_server(
    _cmd_tx: mpsc::Sender<Command>,
    _event_rx: mpsc::Receiver<Event>,
) -> Result<()> {
    log::warn!("Named pipe server only available on Windows");
    Ok(())
}
