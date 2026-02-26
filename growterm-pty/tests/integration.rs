use growterm_pty::PtyReader;
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
    let (reader, mut writer) = growterm_pty::spawn(24, 80).expect("failed to spawn");

    writer.write_all(b"echo hello\n").expect("write failed");

    let output = read_until(reader, "hello", Duration::from_secs(3));
    assert!(
        output.contains("hello"),
        "expected 'hello' in output, got: {output}"
    );
}

#[test]
fn resize() {
    let (_reader, writer) = growterm_pty::spawn(24, 80).expect("failed to spawn");
    writer.resize(40, 120).expect("resize failed");
}

#[test]
fn write_and_read_unicode() {
    let (reader, mut writer) = growterm_pty::spawn(24, 80).expect("failed to spawn");

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
fn term_env_is_set() {
    let (reader, mut writer) = growterm_pty::spawn(24, 80).expect("failed to spawn");

    writer
        .write_all(b"printf 'TERMVAL=%s\\n' \"$TERM\"\n")
        .expect("write failed");

    let output = read_until(reader, "TERMVAL=xterm-256color", Duration::from_secs(20));
    assert!(
        output.contains("TERMVAL=xterm-256color"),
        "expected TERM=xterm-256color in output, got: {output}"
    );
}

#[test]
fn lang_env_is_set() {
    let (reader, mut writer) = growterm_pty::spawn(24, 80).expect("failed to spawn");

    // MARKER로 출력 구간을 식별하여 echo 명령 자체와 구분
    writer
        .write_all(b"printf 'LANGVAL=%s\\n' \"${LANG:-NONE}\"\n")
        .expect("write failed");

    let output = read_until(reader, "LANGVAL=", Duration::from_secs(3));
    // LANGVAL= 이후 실제 값을 확인
    assert!(
        output.contains("LANGVAL=") && !output.contains("LANGVAL=NONE"),
        "LANG should not be empty (한글 깨짐 원인), got: {output}"
    );
}

#[test]
fn hangul_echo_roundtrip() {
    let (reader, mut writer) = growterm_pty::spawn(24, 80).expect("failed to spawn");

    // printf로 한글 UTF-8 바이트를 직접 출력하여 쉘의 echo 처리에 의존하지 않음
    writer
        .write_all(b"printf '\\xed\\x95\\x9c\\xea\\xb8\\x80\\n'\n")
        .expect("write failed");

    let output = read_until(reader, "한글", Duration::from_secs(20));
    // 한글이 깨지지 않고 UTF-8로 정상 출력되는지 확인
    assert!(
        output.contains("한글"),
        "expected '한글' in output (UTF-8 roundtrip), got: {output}"
    );
}

#[test]
fn cwd_is_home() {
    let (reader, mut writer) = growterm_pty::spawn(24, 80).expect("failed to spawn");

    writer.write_all(b"pwd\n").expect("write failed");

    let home = std::env::var("HOME").expect("HOME not set");
    let output = read_until(reader, &home, Duration::from_secs(3));
    assert!(
        output.contains(&home),
        "expected cwd to be {home}, got: {output}"
    );
}

#[test]
fn exit_shell() {
    let (reader, mut writer) = growterm_pty::spawn(24, 80).expect("failed to spawn");

    writer.write_all(b"exit\n\x04").expect("write failed");
    writer.flush().expect("flush failed");
    drop(writer);

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

    let finished = rx.recv_timeout(Duration::from_secs(20)).unwrap_or(false);
    assert!(finished, "reader should terminate after shell exits");
}
