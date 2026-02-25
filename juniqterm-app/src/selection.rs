use juniqterm_types::Cell;

#[derive(Debug, Clone, Copy, Default)]
pub struct Selection {
    pub start: (u16, u16),
    pub end: (u16, u16),
    pub active: bool,
}

impl Selection {
    pub fn begin(&mut self, row: u16, col: u16) {
        self.start = (row, col);
        self.end = (row, col);
        self.active = true;
    }

    pub fn update(&mut self, row: u16, col: u16) {
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
    pub fn normalized(&self) -> ((u16, u16), (u16, u16)) {
        let (s, e) = (self.start, self.end);
        if s.0 < e.0 || (s.0 == e.0 && s.1 <= e.1) {
            (s, e)
        } else {
            (e, s)
        }
    }

    pub fn contains(&self, row: u16, col: u16) -> bool {
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
        for col in col_start..col_end {
            line_text.push(line[col].character);
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
    use juniqterm_types::{Cell, CellFlags, Color};

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
    fn extract_text_partial_line() {
        let cells = make_cells(&["Hello World"]);
        let mut sel = Selection::default();
        sel.start = (0, 6);
        sel.end = (0, 10);
        assert_eq!(extract_text(&cells, &sel), "World");
    }
}
