use std::io::Read;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use growterm_grid::Grid;
use growterm_vt_parser::VtParser;

/// Read PTY output, parse VT sequences, apply to grid until `predicate` returns true or timeout.
fn pty_grid_wait(
    reader: growterm_pty::PtyReader,
    cols: u16,
    rows: u16,
    timeout: Duration,
    predicate: impl Fn(&Grid) -> bool,
) -> Grid {
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

    let mut grid = Grid::new(cols, rows);
    let mut parser = VtParser::new();
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining.min(Duration::from_millis(100))) {
            Ok(data) => {
                let commands = parser.parse(&data);
                for cmd in &commands {
                    grid.apply(cmd);
                }
                if predicate(&grid) {
                    return grid;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    grid
}

/// Extract text from a grid row, trimming trailing spaces.
fn row_text(grid: &Grid, row: usize) -> String {
    grid.cells()[row]
        .iter()
        .map(|c| c.character)
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// Check if any row in the grid contains non-empty text (shell prompt appeared).
fn has_any_content(grid: &Grid) -> bool {
    grid.cells().iter().any(|row| {
        row.iter().any(|cell| cell.character != '\0' && cell.character != ' ')
    })
}

#[test]
fn shell_prompt_appears_on_startup() {
    let (reader, _writer) = growterm_pty::spawn(24, 80).expect("failed to spawn PTY");

    let grid = pty_grid_wait(reader, 80, 24, Duration::from_secs(5), has_any_content);

    // Shell should have printed something (prompt) on the first row
    let first_row = row_text(&grid, 0);
    assert!(
        !first_row.is_empty(),
        "expected shell prompt on first row, but grid was empty"
    );
}

#[test]
fn cursor_is_after_prompt() {
    let (reader, _writer) = growterm_pty::spawn(24, 80).expect("failed to spawn PTY");

    let grid = pty_grid_wait(reader, 80, 24, Duration::from_secs(5), |g| {
        let (_, col) = g.cursor_pos();
        has_any_content(g) && col > 0
    });

    let (_, col) = grid.cursor_pos();
    assert!(
        col > 0,
        "expected cursor after prompt (col > 0), but cursor was at col {col}"
    );
}

#[test]
fn echo_command_output_appears_in_grid() {
    use std::io::Write;

    let (reader, mut writer) = growterm_pty::spawn(24, 80).expect("failed to spawn PTY");

    // Wait for prompt first
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

    let mut grid = Grid::new(80, 24);
    let mut parser = VtParser::new();
    let deadline = Instant::now() + Duration::from_secs(5);

    // Phase 1: wait for prompt
    let mut prompt_ready = false;
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining.min(Duration::from_millis(100))) {
            Ok(data) => {
                let commands = parser.parse(&data);
                for cmd in &commands {
                    grid.apply(cmd);
                }
                if has_any_content(&grid) {
                    prompt_ready = true;
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    assert!(prompt_ready, "shell prompt did not appear");

    // Phase 2: type command and check output
    writer
        .write_all(b"echo GROWTERM_TEST_OK\n")
        .expect("write failed");

    let mut found = false;
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining.min(Duration::from_millis(100))) {
            Ok(data) => {
                let commands = parser.parse(&data);
                for cmd in &commands {
                    grid.apply(cmd);
                }
                // Check all rows for echo output
                for row in 0..24 {
                    if row_text(&grid, row).contains("GROWTERM_TEST_OK") {
                        found = true;
                        break;
                    }
                }
                if found {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    assert!(found, "expected 'GROWTERM_TEST_OK' in grid after echo command");
}
