use juniqterm_grid::Grid;
use juniqterm_vt_parser::VtParser;

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
    assert_eq!(grid.cells()[0][0].fg, juniqterm_types::Color::Indexed(1));
    // 'o' after reset should have default foreground
    assert_eq!(grid.cells()[0][4].fg, juniqterm_types::Color::Default);
}

#[test]
fn cursor_movement_then_overwrite() {
    // Write "abc", move cursor left 2, overwrite with "X"
    let grid = parse_and_apply(b"abc\x1b[2DX", 80, 24);

    assert_eq!(grid.cells()[0][0].character, 'a');
    assert_eq!(grid.cells()[0][1].character, 'X');
    assert_eq!(grid.cells()[0][2].character, 'c');
}
