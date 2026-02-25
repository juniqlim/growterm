use growterm_grid::Grid;
use growterm_vt_parser::VtParser;

fn parse_and_apply(input: &[u8], cols: u16, rows: u16) -> Grid {
    let mut parser = VtParser::new();
    let commands = parser.parse(input);
    let mut grid = Grid::new(cols, rows);
    for cmd in &commands {
        grid.apply(cmd);
    }
    grid
}

fn grid_text(grid: &Grid, row: usize, col_start: usize, len: usize) -> String {
    grid.cells()[row][col_start..col_start + len]
        .iter()
        .map(|c| c.character)
        .collect::<String>()
        .trim_end()
        .to_string()
}

#[test]
fn type_hi() {
    let grid = parse_and_apply(b"hi", 80, 24);

    assert_eq!(grid.cells()[0][0].character, 'h');
    assert_eq!(grid.cells()[0][1].character, 'i');
    assert_eq!(grid.cursor_pos(), (0, 2));
}

#[test]
fn type_ls() {
    let grid = parse_and_apply(b"ls", 80, 24);

    assert_eq!(grid_text(&grid, 0, 0, 2), "ls");
    assert_eq!(grid.cursor_pos(), (0, 2));
}

#[test]
fn type_korean() {
    let grid = parse_and_apply("안녕".as_bytes(), 80, 24);

    assert_eq!(grid.cells()[0][0].character, '안');
    assert_eq!(grid.cells()[0][2].character, '녕');
    assert_eq!(grid.cursor_pos(), (0, 4));
}

#[test]
fn type_hi_enter_ls() {
    let grid = parse_and_apply(b"hi\r\nls", 80, 24);

    assert_eq!(grid_text(&grid, 0, 0, 2), "hi");
    assert_eq!(grid_text(&grid, 1, 0, 2), "ls");
    assert_eq!(grid.cursor_pos(), (1, 2));
}

#[test]
fn colored_text() {
    // ESC[31m = red foreground, ESC[0m = reset
    let grid = parse_and_apply(b"\x1b[31mRED\x1b[0m ok", 80, 24);

    assert_eq!(grid_text(&grid, 0, 0, 6), "RED ok");
    // 'R' should have red foreground
    assert_eq!(grid.cells()[0][0].fg, growterm_types::Color::Indexed(1));
    // 'o' after reset should have default foreground
    assert_eq!(grid.cells()[0][4].fg, growterm_types::Color::Default);
}

#[test]
fn cursor_movement_then_overwrite() {
    // Write "abc", move cursor left 2, overwrite with "X"
    let grid = parse_and_apply(b"abc\x1b[2DX", 80, 24);

    assert_eq!(grid.cells()[0][0].character, 'a');
    assert_eq!(grid.cells()[0][1].character, 'X');
    assert_eq!(grid.cells()[0][2].character, 'c');
}

// === Claude Code style: inverse on/off for status bar ===

#[test]
fn claude_code_inverse_status_bar() {
    // Claude Code renders a status bar like:
    //   ESC[7m (inverse on) " Status " ESC[27m (inverse off) " normal text"
    // Before SGR 27 was implemented, inverse leaked into "normal text".
    let input = b"\x1b[7m Status \x1b[27m normal";
    let grid = parse_and_apply(input, 80, 24);

    // " Status " (cols 0-7) should have INVERSE
    for col in 0..8 {
        assert!(
            grid.cells()[0][col].flags.contains(growterm_types::CellFlags::INVERSE),
            "col {} should be inverse", col,
        );
    }
    // " normal" (cols 8-14) should NOT have INVERSE
    for col in 8..15 {
        assert!(
            !grid.cells()[0][col].flags.contains(growterm_types::CellFlags::INVERSE),
            "col {} should not be inverse", col,
        );
    }
}

#[test]
fn claude_code_bold_inverse_toggle() {
    // Claude Code uses bold+inverse for highlighted items, then resets individually:
    //   ESC[1;7m "highlighted" ESC[27m "bold only" ESC[22m "plain"
    let input = b"\x1b[1;7mHL\x1b[27mBO\x1b[22mPL";
    let grid = parse_and_apply(input, 80, 24);

    use growterm_types::CellFlags;

    // "HL" (cols 0-1): bold + inverse
    assert!(grid.cells()[0][0].flags.contains(CellFlags::BOLD | CellFlags::INVERSE));
    assert!(grid.cells()[0][1].flags.contains(CellFlags::BOLD | CellFlags::INVERSE));

    // "BO" (cols 2-3): bold only, no inverse
    assert!(grid.cells()[0][2].flags.contains(CellFlags::BOLD));
    assert!(!grid.cells()[0][2].flags.contains(CellFlags::INVERSE));
    assert!(grid.cells()[0][3].flags.contains(CellFlags::BOLD));
    assert!(!grid.cells()[0][3].flags.contains(CellFlags::INVERSE));

    // "PL" (cols 4-5): no bold, no inverse
    assert!(!grid.cells()[0][4].flags.contains(CellFlags::BOLD));
    assert!(!grid.cells()[0][4].flags.contains(CellFlags::INVERSE));
}

#[test]
fn claude_code_repeated_inverse_toggle() {
    // Claude Code status line toggles inverse multiple times per line:
    //   ESC[7m"A"ESC[27m"B"ESC[7m"C"ESC[27m"D"
    let input = b"\x1b[7mA\x1b[27mB\x1b[7mC\x1b[27mD";
    let grid = parse_and_apply(input, 80, 24);

    use growterm_types::CellFlags;
    assert!(grid.cells()[0][0].flags.contains(CellFlags::INVERSE));  // A
    assert!(!grid.cells()[0][1].flags.contains(CellFlags::INVERSE)); // B
    assert!(grid.cells()[0][2].flags.contains(CellFlags::INVERSE));  // C
    assert!(!grid.cells()[0][3].flags.contains(CellFlags::INVERSE)); // D
}
