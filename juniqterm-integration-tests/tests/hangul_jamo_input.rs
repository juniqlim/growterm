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

fn parse_dump(content: &str) -> Vec<String> {
    let mut lines = content.lines();
    let _ = lines.next(); // cursor line
    let _ = lines.next(); // "grid:"
    lines.map(|l| l.to_string()).collect()
}

fn wait_for_dump(dump_path: &std::path::Path, timeout: Duration) -> Option<String> {
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
    }
    None
}

fn activate_by_pid(pid: u32) {
    let script = format!(
        r#"tell application "System Events"
            set frontmost of (first process whose unix id is {pid}) to true
        end tell"#
    );
    let _ = Command::new("osascript").arg("-e").arg(&script).output();
    std::thread::sleep(Duration::from_millis(500));
}

/// 앱 실행 → osascript로 한글 자모(ㅎㅏㄴㄱㅡㄹ) 전송 → 엔터 →
/// 그리드에 조합된 "한글"이 나타나는지 검증.
#[test]
fn osascript_jamo_keystroke_produces_composed_hangul() {
    let bin = build_binary();
    let dump_path = std::env::temp_dir().join(format!(
        "juniqterm_jamo_{}.txt",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&dump_path);

    let mut child = Command::new(&bin)
        .env("JUNIQTERM_GRID_DUMP", &dump_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to launch juniqterm");

    let prompt = wait_for_dump(&dump_path, Duration::from_secs(10));
    assert!(prompt.is_some(), "셸 프롬프트가 렌더되지 않음");

    let pid = child.id();
    activate_by_pid(pid);

    let _ = std::fs::remove_file(&dump_path);

    let _ = Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to keystroke \"ㅎㅏㄴㄱㅡㄹ\"")
        .output();

    std::thread::sleep(Duration::from_secs(2));

    let _ = std::fs::remove_file(&dump_path);

    let _ = Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to keystroke return")
        .output();

    let dump_content = wait_for_dump(&dump_path, Duration::from_secs(10));

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(&dump_path);

    let content = dump_content.expect("한글 입력 후 그리드 덤프가 생성되지 않음");
    let rows = parse_dump(&content);
    let all_text: String = rows.join("\n");
    let compact: String = all_text.chars().filter(|c| *c != ' ' && *c != '\0').collect();

    assert!(
        compact.contains("한글"),
        "자모가 조합되지 않음. '한글'이 기대되지만 실제 그리드:\n{all_text}"
    );
}
