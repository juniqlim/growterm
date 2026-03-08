use std::path::{Path, PathBuf};

use growterm_grid::Grid;
use growterm_vt_parser::VtParser;

const DEFAULT_COLS: u16 = 120;
const DEFAULT_ROWS: u16 = 40;
const DEFAULT_FIXTURE: &str = "fixtures/codex-resume.vt";

fn fixture_path() -> PathBuf {
    if let Ok(path) = std::env::var("GROWTERM_CODEX_VT_FIXTURE") {
        return PathBuf::from(path);
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_FIXTURE)
}

fn load_fixture_bytes() -> Vec<u8> {
    let path = fixture_path();
    std::fs::read(&path).unwrap_or_else(|err| {
        panic!(
            "failed to read Codex VT fixture at {}: {err}",
            path.display()
        )
    })
}

fn replay_fixture(bytes: &[u8], cols: u16, rows: u16) -> Grid {
    let mut parser = VtParser::new();
    let mut grid = Grid::new(cols, rows);
    for cmd in parser.parse(bytes) {
        grid.apply(&cmd);
    }
    grid
}

fn infer_rows_from_fixture(bytes: &[u8]) -> u16 {
    let mut max_row = 0u16;
    let mut i = 0usize;
    while i + 2 < bytes.len() {
        if bytes[i] == 0x1b && bytes[i + 1] == b'[' {
            let mut j = i + 2;
            while j < bytes.len() && !(0x40..=0x7e).contains(&bytes[j]) {
                j += 1;
            }
            if j < bytes.len() {
                if matches!(bytes[j], b'H' | b'f' | b'r') {
                    let body = std::str::from_utf8(&bytes[i + 2..j]).unwrap_or("");
                    if let Some((row, _)) = body.split_once(';') {
                        if let Ok(row) = row.trim_start_matches('?').parse::<u16>() {
                            max_row = max_row.max(row);
                        }
                    } else if let Ok(row) = body.trim_start_matches('?').parse::<u16>() {
                        max_row = max_row.max(row);
                    }
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    if max_row == 0 { DEFAULT_ROWS } else { max_row }
}

fn visible_row_text(grid: &Grid, row: usize) -> String {
    grid.visible_cells()[row]
        .iter()
        .map(|cell| cell.character)
        .collect::<String>()
        .trim_end()
        .to_string()
}

fn dump_visible_rows(grid: &Grid) -> String {
    let mut out = String::new();
    for row in 0..grid.visible_cells().len() {
        let line = visible_row_text(grid, row);
        out.push_str(&format!("{row:02}: {line}\n"));
    }
    let (cursor_row, cursor_col) = grid.cursor_pos();
    out.push_str(&format!("cursor: {cursor_row},{cursor_col}\n"));
    out
}

#[test]
#[ignore = "requires a raw Codex VT capture fixture"]
fn codex_resume_fixture_replays_without_parser_panics() {
    let bytes = load_fixture_bytes();
    let rows = infer_rows_from_fixture(&bytes);
    let grid = replay_fixture(&bytes, DEFAULT_COLS, rows);

    assert!(
        grid.visible_cells()
            .iter()
            .flat_map(|row| row.iter())
            .any(|cell| cell.character != ' ' && cell.character != '\0'),
        "fixture replay produced an empty grid\n{}",
        dump_visible_rows(&grid),
    );
}

#[test]
#[ignore = "requires a raw Codex VT capture fixture and concrete assertions"]
fn codex_resume_fixture_status_bar_stays_at_bottom() {
    let bytes = load_fixture_bytes();
    let rows = infer_rows_from_fixture(&bytes);
    let grid = replay_fixture(&bytes, DEFAULT_COLS, rows);
    let rows = grid.visible_cells().len();

    let bottom = visible_row_text(&grid, rows - 1);
    let above_bottom = visible_row_text(&grid, rows.saturating_sub(2));

    assert!(
        bottom.contains("gpt-")
            || bottom.contains("weekly")
            || bottom.contains('%'),
        "expected status bar markers on last row, got:\n{}",
        dump_visible_rows(&grid),
    );
    assert!(
        !above_bottom.contains("gpt-") && !above_bottom.contains("weekly"),
        "status bar appears above the last row\n{}",
        dump_visible_rows(&grid),
    );
}
