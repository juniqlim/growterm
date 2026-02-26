use growterm_app::selection::{self, Selection};
use growterm_grid::Grid;
use growterm_types::TerminalCommand;

fn fill_grid(grid: &mut Grid, lines: &[&str]) {
    for (i, line) in lines.iter().enumerate() {
        for ch in line.chars() {
            grid.apply(&TerminalCommand::Print(ch));
        }
        if i + 1 < lines.len() {
            grid.apply(&TerminalCommand::Newline);
            grid.apply(&TerminalCommand::CarriageReturn);
        }
    }
}

fn fill_with_scrollback(grid: &mut Grid, total_lines: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for i in 0..total_lines {
        let text = format!("line-{:03}", i);
        for ch in text.chars() {
            grid.apply(&TerminalCommand::Print(ch));
        }
        if i + 1 < total_lines {
            grid.apply(&TerminalCommand::Newline);
            grid.apply(&TerminalCommand::CarriageReturn);
        }
        lines.push(text);
    }
    lines
}

fn make_sel(start_row: u32, start_col: u16, end_row: u32, end_col: u16) -> Selection {
    let mut sel = Selection::default();
    sel.start = (start_row, start_col);
    sel.end = (end_row, end_col);
    sel
}

/// Test 1: extract_text_absolute correctly extracts selected screen text
#[test]
fn copy_extracts_correct_text_from_screen() {
    let mut grid = Grid::new(20, 5);
    fill_grid(&mut grid, &["Hello World", "Rust Lang"]);

    let sel = make_sel(0, 0, 0, 4);
    let text = selection::extract_text_absolute(&grid, &sel);
    assert_eq!(text, "Hello");
}

/// Test 2: Changing selection does not change previously extracted text
/// (Simulates: copy "Hello", then select "Rust" without copying)
#[test]
fn selection_change_does_not_affect_previous_copy() {
    let mut grid = Grid::new(20, 5);
    fill_grid(&mut grid, &["Hello World", "Rust Lang"]);

    // Extract "Hello"
    let sel1 = make_sel(0, 0, 0, 4);
    let copied = selection::extract_text_absolute(&grid, &sel1);
    assert_eq!(copied, "Hello");

    // Change selection to "Rust" — but this is a separate extract call
    let sel2 = make_sel(1, 0, 1, 3);
    let new_selection_text = selection::extract_text_absolute(&grid, &sel2);
    assert_eq!(new_selection_text, "Rust");

    // The previously copied text is unchanged
    assert_eq!(copied, "Hello");
}

/// Test 3: extract_text_absolute works on scrollback lines
#[test]
fn copy_extracts_from_scrollback() {
    let mut grid = Grid::new(20, 3);
    let lines = fill_with_scrollback(&mut grid, 6);

    assert!(grid.scrollback_len() > 0);

    // Absolute row 0 = first scrollback line
    let sel = make_sel(0, 0, 0, 7);
    let text = selection::extract_text_absolute(&grid, &sel);
    assert_eq!(text, lines[0]);
}

/// Test 4: Selection spanning scrollback and screen
#[test]
fn copy_spans_scrollback_and_screen() {
    let mut grid = Grid::new(20, 3);
    let lines = fill_with_scrollback(&mut grid, 6);

    let sb_len = grid.scrollback_len() as u32;
    let sel = make_sel(sb_len - 1, 0, sb_len, 7);
    let text = selection::extract_text_absolute(&grid, &sel);

    let expected = format!("{}\n{}", lines[(sb_len - 1) as usize], lines[sb_len as usize]);
    assert_eq!(text, expected);
}

/// Test 5: screen_normalized converts absolute coords to screen coords correctly
#[test]
fn screen_normalized_adjusts_for_scroll() {
    let mut sel = Selection::default();
    sel.start = (5, 2);
    sel.end = (8, 10);

    // View base at absolute row 5, 10 visible rows
    let result = sel.screen_normalized(5, 10);
    assert_eq!(result, Some(((0, 2), (3, 10))));

    // View base at absolute row 10 → selection off-screen
    let result = sel.screen_normalized(10, 10);
    assert_eq!(result, None);
}
