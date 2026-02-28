use growterm_types::{Cell, CellFlags, Color, TerminalCommand};
use unicode_width::UnicodeWidthChar;

const MAX_SCROLLBACK: usize = 10_000;

pub struct Grid {
    cells: Vec<Vec<Cell>>,
    cols: usize,
    rows: usize,
    cursor_row: usize,
    cursor_col: usize,
    current_fg: Color,
    current_bg: Color,
    current_flags: CellFlags,
    scrollback: Vec<Vec<Cell>>,
    scroll_offset: usize,
    cursor_visible: bool,
}

impl Grid {
    pub fn new(cols: u16, rows: u16) -> Self {
        let cols = cols as usize;
        let rows = rows as usize;
        Self {
            cells: vec![vec![Cell::default(); cols]; rows],
            cols,
            rows,
            cursor_row: 0,
            cursor_col: 0,
            current_fg: Color::Default,
            current_bg: Color::Default,
            current_flags: CellFlags::empty(),
            scrollback: Vec::new(),
            scroll_offset: 0,
            cursor_visible: true,
        }
    }

    pub fn cells(&self) -> &[Vec<Cell>] {
        &self.cells
    }

    pub fn cursor_pos(&self) -> (u16, u16) {
        (self.cursor_row as u16, self.cursor_col as u16)
    }

    pub fn cursor_visible(&self) -> bool {
        self.cursor_visible
    }

    pub fn apply(&mut self, cmd: &TerminalCommand) {
        match cmd {
            TerminalCommand::Print(c) => self.print(*c),
            TerminalCommand::CursorUp(n) => {
                self.cursor_row = self.cursor_row.saturating_sub(*n as usize);
            }
            TerminalCommand::CursorDown(n) => {
                self.cursor_row = (self.cursor_row + *n as usize).min(self.rows - 1);
            }
            TerminalCommand::CursorForward(n) => {
                self.cursor_col = (self.cursor_col + *n as usize).min(self.cols - 1);
            }
            TerminalCommand::CursorBack(n) => {
                self.cursor_col = self.cursor_col.saturating_sub(*n as usize);
            }
            TerminalCommand::CursorPosition { row, col } => {
                self.cursor_row = (*row as usize).saturating_sub(1).min(self.rows - 1);
                self.cursor_col = (*col as usize).saturating_sub(1).min(self.cols - 1);
            }
            TerminalCommand::SetForeground(c) => self.current_fg = *c,
            TerminalCommand::SetBackground(c) => self.current_bg = *c,
            TerminalCommand::SetBold => self.current_flags |= CellFlags::BOLD,
            TerminalCommand::SetDim => self.current_flags |= CellFlags::DIM,
            TerminalCommand::SetItalic => self.current_flags |= CellFlags::ITALIC,
            TerminalCommand::SetUnderline => self.current_flags |= CellFlags::UNDERLINE,
            TerminalCommand::SetInverse => self.current_flags |= CellFlags::INVERSE,
            TerminalCommand::SetHidden => self.current_flags |= CellFlags::HIDDEN,
            TerminalCommand::SetStrikethrough => self.current_flags |= CellFlags::STRIKETHROUGH,
            TerminalCommand::ResetBold => self.current_flags.remove(CellFlags::BOLD | CellFlags::DIM),
            TerminalCommand::ResetItalic => self.current_flags.remove(CellFlags::ITALIC),
            TerminalCommand::ResetUnderline => self.current_flags.remove(CellFlags::UNDERLINE),
            TerminalCommand::ResetInverse => self.current_flags.remove(CellFlags::INVERSE),
            TerminalCommand::ResetHidden => self.current_flags.remove(CellFlags::HIDDEN),
            TerminalCommand::ResetStrikethrough => self.current_flags.remove(CellFlags::STRIKETHROUGH),
            TerminalCommand::ResetAttributes => {
                self.current_fg = Color::Default;
                self.current_bg = Color::Default;
                self.current_flags = CellFlags::empty();
            }
            TerminalCommand::Newline => self.newline(),
            TerminalCommand::CarriageReturn => self.cursor_col = 0,
            TerminalCommand::Backspace => {
                self.cursor_col = self.cursor_col.saturating_sub(1);
            }
            TerminalCommand::Tab => {
                self.cursor_col = ((self.cursor_col / 8) + 1) * 8;
                if self.cursor_col >= self.cols {
                    self.cursor_col = self.cols - 1;
                }
            }
            TerminalCommand::Bell => {}
            TerminalCommand::ShowCursor => self.cursor_visible = true,
            TerminalCommand::HideCursor => self.cursor_visible = false,
            TerminalCommand::DeleteChars(n) => self.delete_chars(*n),
            TerminalCommand::EraseInLine(mode) => self.erase_in_line(*mode),
            TerminalCommand::EraseInDisplay(mode) => self.erase_in_display(*mode),
        }
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        let new_cols = cols as usize;
        let new_rows = rows as usize;

        // Adjust existing rows' width
        for row in &mut self.cells {
            row.resize(new_cols, Cell::default());
        }
        // Adjust row count
        self.cells.resize(new_rows, vec![Cell::default(); new_cols]);

        self.cols = new_cols;
        self.rows = new_rows;
        self.cursor_row = self.cursor_row.min(self.rows - 1);
        self.cursor_col = self.cursor_col.min(self.cols - 1);
    }

    fn print(&mut self, c: char) {
        let width = UnicodeWidthChar::width(c).unwrap_or(1);

        if width == 2 {
            // Wide char: need 2 cols. If only 1 remaining, wrap.
            if self.cursor_col + 1 >= self.cols {
                self.wrap_cursor();
            }
        }

        if self.cursor_col >= self.cols {
            self.wrap_cursor();
        }

        // Clean up wide char pairs if overwriting
        self.cleanup_overwrite(self.cursor_row, self.cursor_col);

        let flags = if width == 2 {
            self.current_flags | CellFlags::WIDE_CHAR
        } else {
            self.current_flags
        };

        self.cells[self.cursor_row][self.cursor_col] = Cell {
            character: c,
            fg: self.current_fg,
            bg: self.current_bg,
            flags,
        };
        self.cursor_col += 1;

        if width == 2 {
            // Place spacer cell
            if self.cursor_col < self.cols {
                self.cells[self.cursor_row][self.cursor_col] = Cell::default();
                self.cursor_col += 1;
            }
        }
    }

    fn cleanup_overwrite(&mut self, row: usize, col: usize) {
        let cell = self.cells[row][col];
        // Overwriting the first half of a wide char → clear its spacer
        if cell.flags.contains(CellFlags::WIDE_CHAR) && col + 1 < self.cols {
            self.cells[row][col + 1] = Cell::default();
        }
        // Overwriting a spacer (second half of wide char) → clear the wide char
        if col > 0 && self.cells[row][col - 1].flags.contains(CellFlags::WIDE_CHAR) {
            self.cells[row][col - 1] = Cell::default();
        }
    }

    fn wrap_cursor(&mut self) {
        self.cursor_col = 0;
        if self.cursor_row + 1 >= self.rows {
            self.scroll_up();
        } else {
            self.cursor_row += 1;
        }
    }

    fn newline(&mut self) {
        if self.cursor_row + 1 >= self.rows {
            self.scroll_up();
        } else {
            self.cursor_row += 1;
        }
    }

    fn scroll_up(&mut self) {
        let row = self.cells.remove(0);
        self.scrollback.push(row);
        if self.scrollback.len() > MAX_SCROLLBACK {
            self.scrollback.remove(0);
            // If trimmed and user is scrolled, offset may exceed scrollback
            self.scroll_offset = self.scroll_offset.min(self.scrollback.len());
        }
        self.cells.push(vec![Cell::default(); self.cols]);
        if self.scroll_offset > 0 {
            self.scroll_offset += 1;
            self.scroll_offset = self.scroll_offset.min(self.scrollback.len());
        }
    }

    pub fn scroll_up_view(&mut self, lines: usize) {
        self.scroll_offset = (self.scroll_offset + lines).min(self.scrollback.len());
    }

    pub fn scroll_down_view(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    pub fn reset_scroll(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    pub fn scrollback(&self) -> &[Vec<Cell>] {
        &self.scrollback
    }

    pub fn visible_cells(&self) -> Vec<Vec<Cell>> {
        if self.scroll_offset == 0 {
            return self.cells.clone();
        }
        let sb_len = self.scrollback.len();
        let sb_start = sb_len.saturating_sub(self.scroll_offset);
        let mut result: Vec<Vec<Cell>> = self.scrollback[sb_start..].to_vec();
        let screen_rows_needed = self.rows - result.len().min(self.rows);
        result.extend_from_slice(&self.cells[..screen_rows_needed]);
        result.truncate(self.rows);
        result
    }

    fn blank_cell(&self) -> Cell {
        Cell {
            character: ' ',
            fg: Color::Default,
            bg: self.current_bg,
            flags: CellFlags::empty(),
        }
    }

    fn delete_chars(&mut self, n: u16) {
        let n = n as usize;
        let row = self.cursor_row;
        let col = self.cursor_col;
        let blank = self.blank_cell();
        for i in col..self.cols {
            if i + n < self.cols {
                self.cells[row][i] = self.cells[row][i + n];
            } else {
                self.cells[row][i] = blank;
            }
        }
    }

    fn erase_in_line(&mut self, mode: u16) {
        let row = self.cursor_row;
        let blank = self.blank_cell();
        match mode {
            0 => {
                for col in self.cursor_col..self.cols {
                    self.cells[row][col] = blank;
                }
            }
            1 => {
                for col in 0..=self.cursor_col {
                    self.cells[row][col] = blank;
                }
            }
            2 => {
                for col in 0..self.cols {
                    self.cells[row][col] = blank;
                }
            }
            _ => {}
        }
    }

    fn erase_in_display(&mut self, mode: u16) {
        let blank = self.blank_cell();
        match mode {
            0 => {
                // Erase from cursor to end
                self.erase_in_line(0);
                for row in (self.cursor_row + 1)..self.rows {
                    for col in 0..self.cols {
                        self.cells[row][col] = blank;
                    }
                }
            }
            1 => {
                // Erase from start to cursor
                for row in 0..self.cursor_row {
                    for col in 0..self.cols {
                        self.cells[row][col] = blank;
                    }
                }
                self.erase_in_line(1);
            }
            2 => {
                for row in 0..self.rows {
                    for col in 0..self.cols {
                        self.cells[row][col] = blank;
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "grid_tests.rs"]
mod grid_tests;
