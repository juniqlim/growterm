use std::process::{Command, Stdio};
use std::time::Duration;

use growterm_integration_tests::{build_binary, cleanup, parse_dump_rows, wait_for_dump};

#[test]
fn dropped_file_path_is_inserted_into_prompt() {
    let bin = build_binary();
    let dump_path = std::env::temp_dir().join(format!(
        "growterm_grid_dump_drop_{}.txt",
        std::process::id()
    ));

    let dropped_path = "/tmp/my image.png";
    let mut child = Command::new(&bin)
        .env("GROWTERM_DISABLE_APP_RELAUNCH", "1")
        .env("GROWTERM_GRID_DUMP", &dump_path)
        .env("GROWTERM_TEST_DROPPED_PATH", dropped_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to launch growterm");

    let dump_content = wait_for_dump(&dump_path, Duration::from_secs(10), Some(&mut child));
    if dump_content.is_none() {
        let status = child.try_wait().ok().flatten();
        let stderr = {
            use std::io::Read;
            let mut buf = String::new();
            if let Some(err) = child.stderr.as_mut() {
                let _ = err.read_to_string(&mut buf);
            }
            buf
        };
        cleanup(&mut child, &dump_path);
        panic!(
            "grid dump file was not created within timeout. child_status={status:?}\nstderr:\n{stderr}"
        );
    }
    cleanup(&mut child, &dump_path);

    let content = dump_content.expect("dump already checked");
    let rows = parse_dump_rows(&content);
    let all_text = rows.join("\n");

    assert!(
        all_text.contains("'/tmp/my image.png'"),
        "expected escaped dropped file path in prompt, got:\n{all_text}"
    );
}
