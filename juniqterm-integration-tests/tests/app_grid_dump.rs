use std::process::{Command, Stdio};
use std::time::Duration;

fn build_binary() -> String {
    let output = Command::new("cargo")
        .args(["build", "--package", "juniqterm-app"])
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
    format!("{target_dir}/debug/juniqterm")
}

/// Parse the dump file format:
/// ```
/// cursor:ROW,COL
/// grid:
/// <row 0 text>
/// <row 1 text>
/// ...
/// ```
fn parse_dump(content: &str) -> (u16, u16, Vec<String>) {
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

#[test]
fn shell_prompt_with_home_directory_and_cursor_ready() {
    let bin = build_binary();
    let dump_path = std::env::temp_dir().join(format!(
        "juniqterm_grid_dump_{}.txt",
        std::process::id()
    ));

    let mut child = Command::new(&bin)
        .env("JUNIQTERM_GRID_DUMP", &dump_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to launch juniqterm");

    // Wait for the app to render and write dump file.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut dump_content = None;
    while std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(200));
        if dump_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&dump_path) {
                if !content.is_empty() {
                    dump_content = Some(content);
                    break;
                }
            }
        }
    }

    // Clean up
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(&dump_path);

    let content = dump_content.expect("grid dump file was not created within timeout");
    let (cursor_row, cursor_col, rows) = parse_dump(&content);

    // The first row should contain some prompt text (not empty).
    let first_row = rows.first().expect("no grid rows");
    let first_row_trimmed = first_row.trim_end();
    assert!(
        !first_row_trimmed.is_empty(),
        "expected shell prompt on first row, but it was empty"
    );

    // The home directory path should appear somewhere in the grid.
    let home = std::env::var("HOME").expect("HOME not set");
    let home_short = home.rsplit('/').next().unwrap_or(&home); // e.g. "juniq"
    let all_text: String = rows.join("\n");
    let has_home = all_text.contains(&home) || all_text.contains(home_short) || all_text.contains('~');
    assert!(
        has_home,
        "expected home directory ({home} or ~ or {home_short}) in grid, got:\n{all_text}"
    );

    // Cursor should be positioned after the prompt (col > 0).
    assert!(
        cursor_col > 0,
        "expected cursor after prompt (col > 0), but cursor at row={cursor_row} col={cursor_col}"
    );
}

fn wait_for_dump(child: &mut std::process::Child, dump_path: &std::path::Path) -> Option<String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(200));
        if dump_path.exists() {
            if let Ok(content) = std::fs::read_to_string(dump_path) {
                if !content.is_empty() {
                    return Some(content);
                }
            }
        }
        // Check child hasn't crashed.
        if let Ok(Some(_)) = child.try_wait() {
            return None;
        }
    }
    None
}

#[test]
fn echo_command_output_appears_in_app() {
    let bin = build_binary();
    let dump_path = std::env::temp_dir().join(format!(
        "juniqterm_grid_dump_echo_{}.txt",
        std::process::id()
    ));

    let mut child = Command::new(&bin)
        .env("JUNIQTERM_GRID_DUMP", &dump_path)
        .env("JUNIQTERM_TEST_INPUT", "echo HELLO_JUNIQTERM\n")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to launch juniqterm");

    let dump_content = wait_for_dump(&mut child, &dump_path);

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(&dump_path);

    let content = dump_content.expect("grid dump file was not created within timeout");
    let (_cursor_row, _cursor_col, rows) = parse_dump(&content);

    let all_text: String = rows.join("\n");
    assert!(
        all_text.contains("HELLO_JUNIQTERM"),
        "expected 'HELLO_JUNIQTERM' in grid after echo command, got:\n{all_text}"
    );
}

#[test]
fn echo_korean_output_appears_in_app() {
    let bin = build_binary();
    let dump_path = std::env::temp_dir().join(format!(
        "juniqterm_grid_dump_korean_{}.txt",
        std::process::id()
    ));

    let mut child = Command::new(&bin)
        .env("JUNIQTERM_GRID_DUMP", &dump_path)
        .env("JUNIQTERM_TEST_INPUT", "echo 안녕하세요\n")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to launch juniqterm");

    let dump_content = wait_for_dump(&mut child, &dump_path);

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(&dump_path);

    let content = dump_content.expect("grid dump file was not created within timeout");
    let (_cursor_row, _cursor_col, rows) = parse_dump(&content);

    let all_text: String = rows.join("\n");
    // Wide characters(한글)는 그리드에서 2칸을 차지하므로
    // 공백 제거 후 비교
    let compact: String = all_text.chars().filter(|c| *c != ' ' && *c != '\0').collect();
    assert!(
        compact.contains("안녕하세요"),
        "expected '안녕하세요' in grid after echo command, got:\n{all_text}"
    );
}
