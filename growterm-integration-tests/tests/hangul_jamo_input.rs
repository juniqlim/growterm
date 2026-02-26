use std::process::Command;
use std::time::Duration;

use growterm_integration_tests::{
    build_binary, cleanup, parse_dump_rows, spawn_with_dump, wait_for_dump,
};

fn activate_by_pid(pid: u32) {
    let script = format!(
        r#"tell application "System Events"
            set frontmost of (first process whose unix id is {pid}) to true
        end tell"#
    );
    let _ = Command::new("osascript").arg("-e").arg(&script).output();
    std::thread::sleep(Duration::from_millis(500));
}

fn korean_input_source_selected() -> bool {
    let output = match Command::new("defaults")
        .arg("read")
        .arg("com.apple.HIToolbox")
        .arg("AppleSelectedInputSources")
        .output()
    {
        Ok(output) => output,
        Err(_) => return false,
    };
    if !output.status.success() {
        return false;
    }
    String::from_utf8_lossy(&output.stdout).contains("com.apple.inputmethod.Korean")
}

/// 앱 실행 → osascript로 한글 자모(ㅎㅏㄴㄱㅡㄹ) 전송 → 엔터 →
/// 그리드에 조합된 "한글"이 나타나는지 검증.
#[test]
fn osascript_jamo_keystroke_produces_composed_hangul() {
    if std::env::var("GROWTERM_RUN_IME_COMPOSE_TEST").ok().as_deref() != Some("1") {
        eprintln!("skip: set GROWTERM_RUN_IME_COMPOSE_TEST=1 to run IME composition test");
        return;
    }

    if !korean_input_source_selected() {
        eprintln!("skip: Korean input source is not selected");
        return;
    }

    let bin = build_binary();
    let dump_path = std::env::temp_dir().join(format!(
        "growterm_jamo_{}.txt",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&dump_path);

    let mut child = spawn_with_dump(&bin, &dump_path);

    let prompt = wait_for_dump(&dump_path, Duration::from_secs(10), None);
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

    let dump_content = wait_for_dump(&dump_path, Duration::from_secs(10), None);

    cleanup(&mut child, &dump_path);

    let content = dump_content.expect("한글 입력 후 그리드 덤프가 생성되지 않음");
    let rows = parse_dump_rows(&content);
    let all_text: String = rows.join("\n");
    let compact: String = all_text.chars().filter(|c| *c != ' ' && *c != '\0').collect();

    assert!(
        compact.contains("한글"),
        "자모가 조합되지 않음. '한글'이 기대되지만 실제 그리드:\n{all_text}"
    );
}
