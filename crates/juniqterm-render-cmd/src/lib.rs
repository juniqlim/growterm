use juniqterm_types::{Cell, CellFlags, Color, RenderCommand, Rgb};

const DEFAULT_FG: Rgb = Rgb { r: 204, g: 204, b: 204 };
const DEFAULT_BG: Rgb = Rgb { r: 0, g: 0, b: 0 };

// 256-color palette (indices 0..=255)
const ANSI_COLORS: [Rgb; 16] = [
    Rgb { r: 0, g: 0, b: 0 },         // 0  black
    Rgb { r: 204, g: 0, b: 0 },       // 1  red
    Rgb { r: 0, g: 204, b: 0 },       // 2  green
    Rgb { r: 204, g: 204, b: 0 },     // 3  yellow
    Rgb { r: 0, g: 0, b: 204 },       // 4  blue
    Rgb { r: 204, g: 0, b: 204 },     // 5  magenta
    Rgb { r: 0, g: 204, b: 204 },     // 6  cyan
    Rgb { r: 204, g: 204, b: 204 },   // 7  white
    Rgb { r: 128, g: 128, b: 128 },   // 8  bright black
    Rgb { r: 255, g: 0, b: 0 },       // 9  bright red
    Rgb { r: 0, g: 255, b: 0 },       // 10 bright green
    Rgb { r: 255, g: 255, b: 0 },     // 11 bright yellow
    Rgb { r: 0, g: 0, b: 255 },       // 12 bright blue
    Rgb { r: 255, g: 0, b: 255 },     // 13 bright magenta
    Rgb { r: 0, g: 255, b: 255 },     // 14 bright cyan
    Rgb { r: 255, g: 255, b: 255 },   // 15 bright white
];

fn resolve_color(color: Color, default: Rgb) -> Rgb {
    match color {
        Color::Default => default,
        Color::Rgb(rgb) => rgb,
        Color::Indexed(idx) => {
            if idx < 16 {
                ANSI_COLORS[idx as usize]
            } else if idx < 232 {
                // 216-color cube: 16..=231
                let n = idx - 16;
                let r = (n / 36) % 6;
                let g = (n / 6) % 6;
                let b = n % 6;
                let to_val = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
                Rgb::new(to_val(r), to_val(g), to_val(b))
            } else {
                // Grayscale: 232..=255
                let v = 8 + 10 * (idx - 232);
                Rgb::new(v, v, v)
            }
        }
    }
}

pub fn generate(cells: &[Vec<Cell>], cursor_pos: Option<(u16, u16)>) -> Vec<RenderCommand> {
    let mut commands = Vec::new();
    for (row, line) in cells.iter().enumerate() {
        let mut skip_next = false;
        for (col, cell) in line.iter().enumerate() {
            if skip_next {
                skip_next = false;
                continue;
            }

            let mut fg = resolve_color(cell.fg, DEFAULT_FG);
            let mut bg = resolve_color(cell.bg, DEFAULT_BG);

            // Cursor: swap fg/bg at cursor position
            let is_cursor = cursor_pos == Some((row as u16, col as u16));
            if is_cursor {
                std::mem::swap(&mut fg, &mut bg);
            }

            // INVERSE: swap fg/bg
            if cell.flags.contains(CellFlags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }

            // DIM: halve fg brightness
            if cell.flags.contains(CellFlags::DIM) {
                fg = Rgb::new(fg.r / 2, fg.g / 2, fg.b / 2);
            }

            // HIDDEN: fg = bg
            if cell.flags.contains(CellFlags::HIDDEN) {
                fg = bg;
            }

            commands.push(RenderCommand {
                col: col as u16,
                row: row as u16,
                character: cell.character,
                fg,
                bg,
                flags: cell.flags,
            });

            if cell.flags.contains(CellFlags::WIDE_CHAR) {
                skip_next = true;
            }
        }
    }
    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_grid_produces_no_commands() {
        let cells: Vec<Vec<Cell>> = vec![];
        let cmds = generate(&cells, None);
        assert!(cmds.is_empty());
    }

    #[test]
    fn single_default_cell() {
        let cells = vec![vec![Cell::default()]];
        let cmds = generate(&cells, None);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].col, 0);
        assert_eq!(cmds[0].row, 0);
        assert_eq!(cmds[0].character, ' ');
        assert_eq!(cmds[0].fg, DEFAULT_FG);
        assert_eq!(cmds[0].bg, DEFAULT_BG);
    }

    #[test]
    fn rgb_color_passthrough() {
        let cell = Cell {
            character: 'X',
            fg: Color::Rgb(Rgb::new(100, 150, 200)),
            bg: Color::Rgb(Rgb::new(10, 20, 30)),
            flags: CellFlags::empty(),
        };
        let cmds = generate(&vec![vec![cell]], None);
        assert_eq!(cmds[0].fg, Rgb::new(100, 150, 200));
        assert_eq!(cmds[0].bg, Rgb::new(10, 20, 30));
    }

    #[test]
    fn indexed_color_ansi() {
        let cell = Cell {
            character: 'A',
            fg: Color::Indexed(1), // red
            bg: Color::Indexed(4), // blue
            flags: CellFlags::empty(),
        };
        let cmds = generate(&vec![vec![cell]], None);
        assert_eq!(cmds[0].fg, Rgb::new(204, 0, 0));
        assert_eq!(cmds[0].bg, Rgb::new(0, 0, 204));
    }

    #[test]
    fn indexed_color_216_cube() {
        // Index 196 = 16 + 180 = r=5,g=0,b=0 → (255,0,0)
        let cell = Cell {
            character: 'A',
            fg: Color::Indexed(196),
            bg: Color::Default,
            flags: CellFlags::empty(),
        };
        let cmds = generate(&vec![vec![cell]], None);
        assert_eq!(cmds[0].fg, Rgb::new(255, 0, 0));
    }

    #[test]
    fn indexed_color_grayscale() {
        // Index 232 → 8, Index 255 → 238
        let cell = Cell {
            character: 'A',
            fg: Color::Indexed(232),
            bg: Color::Indexed(255),
            flags: CellFlags::empty(),
        };
        let cmds = generate(&vec![vec![cell]], None);
        assert_eq!(cmds[0].fg, Rgb::new(8, 8, 8));
        assert_eq!(cmds[0].bg, Rgb::new(238, 238, 238));
    }

    #[test]
    fn inverse_swaps_fg_bg() {
        let cell = Cell {
            character: 'I',
            fg: Color::Rgb(Rgb::new(255, 255, 255)),
            bg: Color::Rgb(Rgb::new(0, 0, 0)),
            flags: CellFlags::INVERSE,
        };
        let cmds = generate(&vec![vec![cell]], None);
        assert_eq!(cmds[0].fg, Rgb::new(0, 0, 0));
        assert_eq!(cmds[0].bg, Rgb::new(255, 255, 255));
    }

    #[test]
    fn dim_halves_fg() {
        let cell = Cell {
            character: 'D',
            fg: Color::Rgb(Rgb::new(200, 100, 50)),
            bg: Color::Default,
            flags: CellFlags::DIM,
        };
        let cmds = generate(&vec![vec![cell]], None);
        assert_eq!(cmds[0].fg, Rgb::new(100, 50, 25));
    }

    #[test]
    fn hidden_sets_fg_to_bg() {
        let cell = Cell {
            character: 'H',
            fg: Color::Rgb(Rgb::new(255, 255, 255)),
            bg: Color::Rgb(Rgb::new(0, 0, 0)),
            flags: CellFlags::HIDDEN,
        };
        let cmds = generate(&vec![vec![cell]], None);
        assert_eq!(cmds[0].fg, cmds[0].bg);
    }

    #[test]
    fn wide_char_with_spacer_skips_spacer() {
        // Fixed-width grid format: wide char + spacer cell
        let cells = vec![vec![
            Cell {
                character: '한',
                fg: Color::Default,
                bg: Color::Default,
                flags: CellFlags::WIDE_CHAR,
            },
            Cell::default(), // spacer
            Cell {
                character: '글',
                fg: Color::Default,
                bg: Color::Default,
                flags: CellFlags::WIDE_CHAR,
            },
            Cell::default(), // spacer
        ]];
        let cmds = generate(&cells, None);
        assert_eq!(cmds.len(), 2); // spacers skipped
        assert_eq!(cmds[0].col, 0);
        assert_eq!(cmds[0].character, '한');
        assert_eq!(cmds[1].col, 2);
        assert_eq!(cmds[1].character, '글');
    }

    #[test]
    fn multiple_rows() {
        let cells = vec![
            vec![Cell { character: 'A', ..Cell::default() }],
            vec![Cell { character: 'B', ..Cell::default() }],
            vec![Cell { character: 'C', ..Cell::default() }],
        ];
        let cmds = generate(&cells, None);
        assert_eq!(cmds.len(), 3);
        assert_eq!(cmds[0].row, 0);
        assert_eq!(cmds[1].row, 1);
        assert_eq!(cmds[2].row, 2);
    }

    #[test]
    fn cursor_pos_swaps_fg_bg() {
        let cell = Cell {
            character: 'A',
            fg: Color::Default,
            bg: Color::Default,
            flags: CellFlags::empty(),
        };
        let cells = vec![vec![cell]];
        let cmds = generate(&cells, Some((0, 0)));
        // fg and bg should be swapped at cursor position
        assert_eq!(cmds[0].fg, DEFAULT_BG);
        assert_eq!(cmds[0].bg, DEFAULT_FG);
    }

    #[test]
    fn cursor_pos_only_affects_cursor_cell() {
        let cells = vec![vec![
            Cell { character: 'A', ..Cell::default() },
            Cell { character: 'B', ..Cell::default() },
        ]];
        let cmds = generate(&cells, Some((0, 0)));
        // Cell at cursor: swapped
        assert_eq!(cmds[0].fg, DEFAULT_BG);
        assert_eq!(cmds[0].bg, DEFAULT_FG);
        // Cell not at cursor: normal
        assert_eq!(cmds[1].fg, DEFAULT_FG);
        assert_eq!(cmds[1].bg, DEFAULT_BG);
    }

    #[test]
    fn flags_are_preserved() {
        let cell = Cell {
            character: 'B',
            fg: Color::Default,
            bg: Color::Default,
            flags: CellFlags::BOLD | CellFlags::UNDERLINE,
        };
        let cmds = generate(&vec![vec![cell]], None);
        assert!(cmds[0].flags.contains(CellFlags::BOLD));
        assert!(cmds[0].flags.contains(CellFlags::UNDERLINE));
    }
}
