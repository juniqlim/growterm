use std::time::Duration;

use growterm_integration_tests::{
    build_binary, cleanup, parse_dump, spawn_with_dump, spawn_with_dump_and_input, wait_for_dump,
};

#[test]
fn shell_prompt_with_home_directory_and_cursor_ready() {
    let bin = build_binary();
    let dump_path = std::env::temp_dir().join(format!(
        "growterm_grid_dump_{}.txt",
        std::process::id()
    ));

    let mut child = spawn_with_dump(&bin, &dump_path);

    let dump_content = wait_for_dump(&dump_path, Duration::from_secs(10), Some(&mut child));

    cleanup(&mut child, &dump_path);

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

#[test]
fn echo_command_output_appears_in_app() {
    let bin = build_binary();
    let dump_path = std::env::temp_dir().join(format!(
        "growterm_grid_dump_echo_{}.txt",
        std::process::id()
    ));

    let mut child = spawn_with_dump_and_input(&bin, &dump_path, "echo HELLO_GROWTERM\n");

    let dump_content = wait_for_dump(&dump_path, Duration::from_secs(10), Some(&mut child));

    cleanup(&mut child, &dump_path);

    let content = dump_content.expect("grid dump file was not created within timeout");
    let (_, _, rows) = parse_dump(&content);

    let all_text: String = rows.join("\n");
    assert!(
        all_text.contains("HELLO_GROWTERM"),
        "expected 'HELLO_GROWTERM' in grid after echo command, got:\n{all_text}"
    );
}

#[test]
fn echo_korean_output_appears_in_app() {
    let bin = build_binary();
    let dump_path = std::env::temp_dir().join(format!(
        "growterm_grid_dump_korean_{}.txt",
        std::process::id()
    ));

    let mut child = spawn_with_dump_and_input(&bin, &dump_path, "echo 안녕하세요\n");

    let dump_content = wait_for_dump(&dump_path, Duration::from_secs(10), Some(&mut child));

    cleanup(&mut child, &dump_path);

    let content = dump_content.expect("grid dump file was not created within timeout");
    let (_, _, rows) = parse_dump(&content);

    let all_text: String = rows.join("\n");
    // Wide characters(한글)는 그리드에서 2칸을 차지하므로
    // 공백 제거 후 비교
    let compact: String = all_text.chars().filter(|c| *c != ' ' && *c != '\0').collect();
    assert!(
        compact.contains("안녕하세요"),
        "expected '안녕하세요' in grid after echo command, got:\n{all_text}"
    );
}
