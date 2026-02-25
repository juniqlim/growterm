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

/// 앱을 실행하고 osascript로 한글 자모(ㅎㅏㄴㄱㅡㄹ)를 키스트로크로 보낸 뒤,
/// 그리드에 조합된 "한글"이 나타나는지 검증한다.
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

    // 1단계: 셸 프롬프트 대기
    let prompt = wait_for_dump(&dump_path, Duration::from_secs(10));
    assert!(prompt.is_some(), "셸 프롬프트가 렌더되지 않음");

    // 2단계: PID 기반으로 포커스 → 덤프 파일 삭제 → 한글 자모 전송
    let pid = child.id();
    let activate_script = format!(
        r#"tell application "System Events"
            set frontmost of (first process whose unix id is {pid}) to true
        end tell"#
    );
    let _ = Command::new("osascript")
        .arg("-e")
        .arg(&activate_script)
        .output();
    std::thread::sleep(Duration::from_millis(500));

    let _ = std::fs::remove_file(&dump_path);

    let osascript_result = Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to keystroke \"ㅎㅏㄴㄱㅡㄹ\"")
        .output()
        .expect("failed to run osascript");

    if !osascript_result.status.success() {
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_file(&dump_path);
        panic!(
            "osascript 실패: {}",
            String::from_utf8_lossy(&osascript_result.stderr)
        );
    }

    std::thread::sleep(Duration::from_secs(2));

    // 3단계: 엔터를 쳐서 셸 출력 유도 후 새 덤프 대기
    let _ = std::fs::remove_file(&dump_path);

    let _ = Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to keystroke return")
        .output();

    let dump_content = wait_for_dump(&dump_path, Duration::from_secs(10));

    // 정리
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
