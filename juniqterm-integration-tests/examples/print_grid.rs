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

fn print_grid(grid: &Grid, rows: usize, cols: usize) {
    for r in 0..rows {
        let line: String = grid.cells()[r][..cols]
            .iter()
            .map(|c| c.character)
            .collect();
        println!("row {}: |{}|", r, line.trim_end());
    }
    let (row, col) = grid.cursor_pos();
    println!("cursor: row={}, col={}", row, col);
}

fn main() {
    println!("=== 정상: \"안녕\" ===");
    let grid = parse_and_apply("안녕".as_bytes(), 20, 3);
    print_grid(&grid, 1, 20);

    println!();
    println!("=== 버그: 첫 자소 분리 \"ㅇ\" + \"ㅏㄴ녕\" ===");
    let grid = parse_and_apply("ㅇㅏㄴ녕".as_bytes(), 20, 3);
    print_grid(&grid, 1, 20);
}
