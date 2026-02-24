use juniqterm_pty::PtyReader;
use std::io::{Read, Write};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Helper: spawn a reader thread, collect output until `target` appears or timeout.
fn read_until(reader: PtyReader, target: &str, timeout: Duration) -> String {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let mut reader = reader;
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut output = String::new();
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining.min(Duration::from_millis(100))) {
            Ok(data) => {
                output.push_str(&String::from_utf8_lossy(&data));
                if output.contains(target) {
                    return output;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    output
}

#[test]
fn spawn_and_echo() {
    let (reader, mut writer) = juniqterm_pty::spawn(24, 80).expect("failed to spawn");

    writer.write_all(b"echo hello\n").expect("write failed");

    let output = read_until(reader, "hello", Duration::from_secs(3));
    assert!(
        output.contains("hello"),
        "expected 'hello' in output, got: {output}"
    );
}

#[test]
fn resize() {
    let (_reader, writer) = juniqterm_pty::spawn(24, 80).expect("failed to spawn");
    writer.resize(40, 120).expect("resize failed");
}

#[test]
fn write_and_read_unicode() {
    let (reader, mut writer) = juniqterm_pty::spawn(24, 80).expect("failed to spawn");

    writer
        .write_all("echo 한글\n".as_bytes())
        .expect("write failed");

    let output = read_until(reader, "한글", Duration::from_secs(3));
    assert!(
        output.contains("한글"),
        "expected '한글' in output, got: {output}"
    );
}

#[test]
fn exit_shell() {
    let (reader, mut writer) = juniqterm_pty::spawn(24, 80).expect("failed to spawn");

    writer.write_all(b"exit\n").expect("write failed");

    // After exit, reader should eventually get EOF or error
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let mut reader = reader;
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    tx.send(true).ok();
                    break;
                }
                Ok(_) => continue,
                Err(_) => {
                    tx.send(true).ok();
                    break;
                }
            }
        }
    });

    let finished = rx.recv_timeout(Duration::from_secs(5)).unwrap_or(false);
    assert!(finished, "reader should terminate after shell exits");
}
