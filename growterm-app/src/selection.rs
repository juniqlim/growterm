use growterm_types::{Cell, CellFlags};

#[derive(Debug, Clone, Copy, Default)]
pub struct Selection {
    /// Absolute row (scrollback + screen), column
    pub start: (u32, u16),
    pub end: (u32, u16),
    pub active: bool,
}

impl Selection {
    pub fn begin(&mut self, row: u32, col: u16) {
        self.start = (row, col);
        self.end = (row, col);
        self.active = true;
    }

    pub fn update(&mut self, row: u32, col: u16) {
        self.end = (row, col);
    }

    pub fn finish(&mut self) {
        self.active = false;
    }

    pub fn clear(&mut self) {
        self.active = false;
        self.start = (0, 0);
        self.end = (0, 0);
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns (start, end) in normalized order (top-left to bottom-right)
    pub fn normalized(&self) -> ((u32, u16), (u32, u16)) {
        let (s, e) = (self.start, self.end);
        if s.0 < e.0 || (s.0 == e.0 && s.1 <= e.1) {
            (s, e)
        } else {
            (e, s)
        }
    }

    /// Convert absolute selection to screen-relative for rendering.
    /// Returns None if the selection is entirely off-screen.
    pub fn screen_normalized(&self, view_base: u32, visible_rows: u16) -> Option<((u16, u16), (u16, u16))> {
        if self.is_empty() {
            return None;
        }
        let ((sr, sc), (er, ec)) = self.normalized();
        let view_end = view_base + visible_rows as u32;
        // Entirely off-screen?
        if er < view_base || sr >= view_end {
            return None;
        }
        let screen_sr = if sr >= view_base { (sr - view_base) as u16 } else { 0 };
        let screen_sc = if sr >= view_base { sc } else { 0 };
        let screen_er = if er < view_end { (er - view_base) as u16 } else { visible_rows - 1 };
        let screen_ec = if er < view_end { ec } else { u16::MAX };
        Some(((screen_sr, screen_sc), (screen_er, screen_ec)))
    }

    pub fn contains(&self, row: u32, col: u16) -> bool {
        if self.is_empty() {
            return false;
        }
        let ((sr, sc), (er, ec)) = self.normalized();
        if row < sr || row > er {
            return false;
        }
        if sr == er {
            return col >= sc && col <= ec;
        }
        if row == sr {
            return col >= sc;
        }
        if row == er {
            return col <= ec;
        }
        true
    }
}

pub fn pixel_to_cell(x: f32, y: f32, cell_w: f32, cell_h: f32) -> (u16, u16) {
    let col = (x / cell_w).floor().max(0.0) as u16;
    let row = (y / cell_h).floor().max(0.0) as u16;
    (row, col)
}

pub fn extract_text(cells: &[Vec<Cell>], selection: &Selection) -> String {
    if selection.is_empty() {
        return String::new();
    }
    let ((sr, sc), (er, ec)) = selection.normalized();
    let mut result = String::new();

    for row in sr..=er {
        let row_idx = row as usize;
        if row_idx >= cells.len() {
            break;
        }
        let line = &cells[row_idx];
        let col_start = if row == sr { sc as usize } else { 0 };
        let col_end = if row == er {
            (ec as usize + 1).min(line.len())
        } else {
            line.len()
        };

        let mut line_text = String::new();
        let mut col = col_start;
        while col < col_end {
            line_text.push(line[col].character);
            if line[col].flags.contains(CellFlags::WIDE_CHAR) {
                col += 2;
            } else {
                col += 1;
            }
        }
        let trimmed = line_text.trim_end();
        result.push_str(trimmed);

        if row < er {
            result.push('\n');
        }
    }
    result
}

/// Extract text using absolute row coordinates from scrollback + screen cells.
pub fn extract_text_absolute(grid: &growterm_grid::Grid, selection: &Selection) -> String {
    if selection.is_empty() {
        return String::new();
    }
    let ((sr, sc), (er, ec)) = selection.normalized();
    let scrollback = grid.scrollback();
    let screen = grid.cells();
    let sb_len = scrollback.len() as u32;
    let mut result = String::new();

    for row in sr..=er {
        let line: &[Cell] = if row < sb_len {
            &scrollback[row as usize]
        } else {
            let screen_row = (row - sb_len) as usize;
            if screen_row >= screen.len() {
                break;
            }
            &screen[screen_row]
        };
        let col_start = if row == sr { sc as usize } else { 0 };
        let col_end = if row == er {
            (ec as usize + 1).min(line.len())
        } else {
            line.len()
        };

        let mut line_text = String::new();
        let mut col = col_start;
        while col < col_end {
            line_text.push(line[col].character);
            if line[col].flags.contains(CellFlags::WIDE_CHAR) {
                col += 2;
            } else {
                col += 1;
            }
        }
        let trimmed = line_text.trim_end();
        result.push_str(trimmed);

        if row < er {
            result.push('\n');
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use growterm_types::{Cell, CellFlags, Color};

    #[test]
    fn pixel_to_cell_basic() {
        assert_eq!(pixel_to_cell(0.0, 0.0, 10.0, 20.0), (0, 0));
        assert_eq!(pixel_to_cell(15.0, 25.0, 10.0, 20.0), (1, 1));
        assert_eq!(pixel_to_cell(29.9, 59.9, 10.0, 20.0), (2, 2));
    }

    #[test]
    fn pixel_to_cell_negative_clamped() {
        assert_eq!(pixel_to_cell(-5.0, -10.0, 10.0, 20.0), (0, 0));
    }

    #[test]
    fn contains_single_row() {
        let mut sel = Selection::default();
        sel.start = (0, 2);
        sel.end = (0, 5);
        assert!(!sel.contains(0, 1));
        assert!(sel.contains(0, 2));
        assert!(sel.contains(0, 3));
        assert!(sel.contains(0, 5));
        assert!(!sel.contains(0, 6));
        assert!(!sel.contains(1, 3));
    }

    #[test]
    fn contains_multi_row() {
        let mut sel = Selection::default();
        sel.start = (1, 3);
        sel.end = (3, 2);
        assert!(!sel.contains(0, 5));
        assert!(!sel.contains(1, 2));
        assert!(sel.contains(1, 3));
        assert!(sel.contains(1, 10));
        assert!(sel.contains(2, 0));
        assert!(sel.contains(2, 50));
        assert!(sel.contains(3, 0));
        assert!(sel.contains(3, 2));
        assert!(!sel.contains(3, 3));
        assert!(!sel.contains(4, 0));
    }

    #[test]
    fn contains_reversed_selection() {
        let mut sel = Selection::default();
        sel.start = (3, 2);
        sel.end = (1, 3);
        assert!(sel.contains(1, 3));
        assert!(sel.contains(2, 0));
        assert!(sel.contains(3, 2));
        assert!(!sel.contains(3, 3));
    }

    #[test]
    fn contains_empty_selection() {
        let mut sel = Selection::default();
        sel.start = (1, 1);
        sel.end = (1, 1);
        assert!(!sel.contains(1, 1));
    }

    fn make_cells(lines: &[&str]) -> Vec<Vec<Cell>> {
        lines
            .iter()
            .map(|s| {
                s.chars()
                    .map(|c| Cell {
                        character: c,
                        fg: Color::Default,
                        bg: Color::Default,
                        flags: CellFlags::empty(),
                    })
                    .collect()
            })
            .collect()
    }

    /// Build cells like the grid does: wide chars get WIDE_CHAR flag + spacer cell
    fn make_cells_with_wide(lines: &[&str]) -> Vec<Vec<Cell>> {
        use unicode_width::UnicodeWidthChar;
        lines
            .iter()
            .map(|s| {
                let mut row = Vec::new();
                for c in s.chars() {
                    let w = UnicodeWidthChar::width(c).unwrap_or(1);
                    row.push(Cell {
                        character: c,
                        fg: Color::Default,
                        bg: Color::Default,
                        flags: if w == 2 { CellFlags::WIDE_CHAR } else { CellFlags::empty() },
                    });
                    if w == 2 {
                        row.push(Cell::default()); // spacer
                    }
                }
                row
            })
            .collect()
    }

    #[test]
    fn extract_text_single_line() {
        let cells = make_cells(&["Hello World"]);
        let mut sel = Selection::default();
        sel.start = (0, 0);
        sel.end = (0, 4);
        assert_eq!(extract_text(&cells, &sel), "Hello");
    }

    #[test]
    fn extract_text_multi_line() {
        let cells = make_cells(&["Hello  ", "World  "]);
        let mut sel = Selection::default();
        sel.start = (0, 0);
        sel.end = (1, 4);
        assert_eq!(extract_text(&cells, &sel), "Hello\nWorld");
    }

    #[test]
    fn extract_text_trims_trailing_spaces() {
        let cells = make_cells(&["Hi   "]);
        let mut sel = Selection::default();
        sel.start = (0, 0);
        sel.end = (0, 4);
        assert_eq!(extract_text(&cells, &sel), "Hi");
    }

    #[test]
    fn extract_text_empty_selection() {
        let cells = make_cells(&["Hello"]);
        let sel = Selection::default();
        assert_eq!(extract_text(&cells, &sel), "");
    }

    #[test]
    fn extract_text_wide_chars_no_spaces() {
        let cells = make_cells_with_wide(&["안녕하세요"]);
        let mut sel = Selection::default();
        sel.start = (0, 0);
        sel.end = (0, 9);
        assert_eq!(extract_text(&cells, &sel), "안녕하세요");
    }

    #[test]
    fn extract_text_mixed_ascii_and_wide() {
        let cells = make_cells_with_wide(&["Hi한글ok"]);
        let mut sel = Selection::default();
        sel.start = (0, 0);
        sel.end = (0, 7);
        assert_eq!(extract_text(&cells, &sel), "Hi한글ok");
    }

    #[test]
    fn extract_text_partial_line() {
        let cells = make_cells(&["Hello World"]);
        let mut sel = Selection::default();
        sel.start = (0, 6);
        sel.end = (0, 10);
        assert_eq!(extract_text(&cells, &sel), "World");
    }

    #[test]
    fn screen_normalized_basic() {
        let mut sel = Selection::default();
        sel.start = (10, 2);
        sel.end = (12, 5);
        // view_base=10, 24 visible rows
        let result = sel.screen_normalized(10, 24);
        assert_eq!(result, Some(((0, 2), (2, 5))));
    }

    #[test]
    fn screen_normalized_off_screen() {
        let mut sel = Selection::default();
        sel.start = (0, 0);
        sel.end = (5, 3);
        // view starts at row 10
        let result = sel.screen_normalized(10, 24);
        assert_eq!(result, None);
    }

    #[test]
    fn screen_normalized_partial_overlap() {
        let mut sel = Selection::default();
        sel.start = (8, 3);
        sel.end = (12, 5);
        // view_base=10, 24 visible rows -> selection starts before view
        let result = sel.screen_normalized(10, 24);
        assert_eq!(result, Some(((0, 0), (2, 5))));
    }
}
