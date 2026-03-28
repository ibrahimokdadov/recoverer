// engine/tests/test_integration.rs
/// Integration test: start engine, connect via pipe, send Ping, receive Pong.
/// Requires Windows. Run with: cargo test test_integration -- --ignored

#[cfg(target_os = "windows")]
#[test]
#[ignore]
fn pipe_ping_pong() {
    use std::io::{BufRead, BufReader, Write};

    // Start engine in background
    let mut child = std::process::Command::new("cargo")
        .args(["run", "--release", "-p", "recoverer-engine"])
        .spawn()
        .expect("failed to start engine");

    std::thread::sleep(std::time::Duration::from_secs(2));

    // Connect to named pipe
    let pipe = std::fs::OpenOptions::new()
        .read(true).write(true)
        .open(r"\\.\pipe\recoverer-engine")
        .expect("failed to connect to pipe");

    let mut writer = std::io::BufWriter::new(&pipe);
    let mut reader = BufReader::new(&pipe);

    writer.write_all(b"{\"type\":\"Ping\"}\n").unwrap();
    writer.flush().unwrap();

    let mut response = String::new();
    reader.read_line(&mut response).unwrap();
    assert!(response.contains("Pong"), "Expected Pong, got: {}", response);

    child.kill().ok();
}
