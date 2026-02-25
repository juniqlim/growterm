use std::process::{Command, Stdio};
use std::time::Duration;

/// Build the growterm binary and return the path.
pub fn build_binary() -> String {
    let output = Command::new("cargo")
        .args(["build", "--package", "growterm-app"])
        .output()
        .expect("failed to run cargo build");
    assert!(
        output.status.success(),
        "cargo build failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let metadata = Command::new("cargo")
        .args(["metadata", "--format-version=1", "--no-deps"])
        .output()
        .expect("failed to run cargo metadata");
    let meta: serde_json::Value =
        serde_json::from_slice(&metadata.stdout).expect("invalid cargo metadata json");
    let target_dir = meta["target_directory"].as_str().unwrap();
    format!("{target_dir}/debug/growterm")
}

/// Parse the dump file format, returning (cursor_row, cursor_col, grid_rows).
pub fn parse_dump(content: &str) -> (u16, u16, Vec<String>) {
    let mut lines = content.lines();

    let cursor_line = lines.next().expect("missing cursor line");
    let cursor_part = cursor_line.strip_prefix("cursor:").expect("bad cursor line");
    let mut parts = cursor_part.split(',');
    let row: u16 = parts.next().unwrap().parse().unwrap();
    let col: u16 = parts.next().unwrap().parse().unwrap();

    let grid_header = lines.next().expect("missing grid header");
    assert_eq!(grid_header, "grid:");

    let rows: Vec<String> = lines.map(|l| l.to_string()).collect();
    (row, col, rows)
}

/// Parse the dump file, returning only grid rows (ignoring cursor).
pub fn parse_dump_rows(content: &str) -> Vec<String> {
    let (_, _, rows) = parse_dump(content);
    rows
}

/// Poll for dump file with timeout. Optionally checks if child has crashed.
pub fn wait_for_dump(
    dump_path: &std::path::Path,
    timeout: Duration,
    mut child: Option<&mut std::process::Child>,
) -> Option<String> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(200));
        if dump_path.exists() {
            if let Ok(content) = std::fs::read_to_string(dump_path) {
                if !content.is_empty() {
                    return Some(content);
                }
            }
        }
        if let Some(ref mut c) = child {
            if let Ok(Some(_)) = c.try_wait() {
                return None;
            }
        }
    }
    None
}

/// Kill child process and remove dump file.
pub fn cleanup(child: &mut std::process::Child, dump_path: &std::path::Path) {
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(dump_path);
}

/// Spawn growterm with GROWTERM_GRID_DUMP set.
pub fn spawn_with_dump(bin: &str, dump_path: &std::path::Path) -> std::process::Child {
    Command::new(bin)
        .env("GROWTERM_GRID_DUMP", dump_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to launch growterm")
}

/// Spawn growterm with GROWTERM_GRID_DUMP and GROWTERM_TEST_INPUT set.
pub fn spawn_with_dump_and_input(
    bin: &str,
    dump_path: &std::path::Path,
    test_input: &str,
) -> std::process::Child {
    Command::new(bin)
        .env("GROWTERM_GRID_DUMP", dump_path)
        .env("GROWTERM_TEST_INPUT", test_input)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to launch growterm")
}
