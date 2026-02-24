use juniqterm_types::{Color, Rgb, TerminalCommand};

struct Handler {
    commands: Vec<TerminalCommand>,
}

impl Handler {
    fn new() -> Self {
        Self { commands: Vec::new() }
    }

    fn take(&mut self) -> Vec<TerminalCommand> {
        std::mem::take(&mut self.commands)
    }

    fn handle_sgr(&mut self, params: &vte::Params) {
        let mut iter = params.iter();
        loop {
            let param = match iter.next() {
                Some(p) => p[0],
                None => return,
            };
            match param {
                0 => self.commands.push(TerminalCommand::ResetAttributes),
                1 => self.commands.push(TerminalCommand::SetBold),
                2 => self.commands.push(TerminalCommand::SetDim),
                3 => self.commands.push(TerminalCommand::SetItalic),
                4 => self.commands.push(TerminalCommand::SetUnderline),
                7 => self.commands.push(TerminalCommand::SetInverse),
                8 => self.commands.push(TerminalCommand::SetHidden),
                9 => self.commands.push(TerminalCommand::SetStrikethrough),
                // Standard foreground colors 30-37
                30..=37 => {
                    self.commands.push(TerminalCommand::SetForeground(
                        Color::Indexed((param - 30) as u8),
                    ));
                }
                38 => {
                    if let Some(color) = self.parse_extended_color(&mut iter) {
                        self.commands.push(TerminalCommand::SetForeground(color));
                    }
                }
                39 => self.commands.push(TerminalCommand::SetForeground(Color::Default)),
                // Standard background colors 40-47
                40..=47 => {
                    self.commands.push(TerminalCommand::SetBackground(
                        Color::Indexed((param - 40) as u8),
                    ));
                }
                48 => {
                    if let Some(color) = self.parse_extended_color(&mut iter) {
                        self.commands.push(TerminalCommand::SetBackground(color));
                    }
                }
                49 => self.commands.push(TerminalCommand::SetBackground(Color::Default)),
                // Bright foreground colors 90-97
                90..=97 => {
                    self.commands.push(TerminalCommand::SetForeground(
                        Color::Indexed((param - 90 + 8) as u8),
                    ));
                }
                // Bright background colors 100-107
                100..=107 => {
                    self.commands.push(TerminalCommand::SetBackground(
                        Color::Indexed((param - 100 + 8) as u8),
                    ));
                }
                _ => {} // ignore unknown SGR
            }
        }
    }

    fn parse_extended_color<'a>(
        &self,
        iter: &mut impl Iterator<Item = &'a [u16]>,
    ) -> Option<Color> {
        match iter.next()?.first()? {
            5 => {
                // 256-color: 38;5;N or 48;5;N
                let idx = *iter.next()?.first()? as u8;
                Some(Color::Indexed(idx))
            }
            2 => {
                // RGB: 38;2;R;G;B or 48;2;R;G;B
                let r = *iter.next()?.first()? as u8;
                let g = *iter.next()?.first()? as u8;
                let b = *iter.next()?.first()? as u8;
                Some(Color::Rgb(Rgb::new(r, g, b)))
            }
            _ => None,
        }
    }
}

impl vte::Perform for Handler {
    fn print(&mut self, c: char) {
        self.commands.push(TerminalCommand::Print(c));
    }

    fn execute(&mut self, byte: u8) {
        let cmd = match byte {
            0x07 => TerminalCommand::Bell,
            0x08 => TerminalCommand::Backspace,
            0x09 => TerminalCommand::Tab,
            0x0A => TerminalCommand::Newline,
            0x0D => TerminalCommand::CarriageReturn,
            _ => return,
        };
        self.commands.push(cmd);
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        let first = params.iter().next().map(|p| p[0]).unwrap_or(0);
        match action {
            'A' => self.commands.push(TerminalCommand::CursorUp(first.max(1))),
            'B' => self.commands.push(TerminalCommand::CursorDown(first.max(1))),
            'C' => self.commands.push(TerminalCommand::CursorForward(first.max(1))),
            'D' => self.commands.push(TerminalCommand::CursorBack(first.max(1))),
            'H' | 'f' => {
                let mut p = params.iter();
                let row = p.next().map(|v| v[0]).unwrap_or(0).max(1);
                let col = p.next().map(|v| v[0]).unwrap_or(0).max(1);
                self.commands.push(TerminalCommand::CursorPosition { row, col });
            }
            'J' => self.commands.push(TerminalCommand::EraseInDisplay(first)),
            'K' => self.commands.push(TerminalCommand::EraseInLine(first)),
            'm' => self.handle_sgr(params),
            _ => {} // ignore unknown CSI
        }
    }
}

pub struct VtParser {
    parser: vte::Parser,
    handler: Handler,
}

impl VtParser {
    pub fn new() -> Self {
        Self {
            parser: vte::Parser::new(),
            handler: Handler::new(),
        }
    }

    pub fn parse(&mut self, bytes: &[u8]) -> Vec<TerminalCommand> {
        for &byte in bytes {
            self.parser.advance(&mut self.handler, byte);
        }
        self.handler.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- ASCII text ---

    #[test]
    fn parse_ascii_text() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"Hello");
        assert_eq!(cmds, vec![
            TerminalCommand::Print('H'),
            TerminalCommand::Print('e'),
            TerminalCommand::Print('l'),
            TerminalCommand::Print('l'),
            TerminalCommand::Print('o'),
        ]);
    }

    #[test]
    fn parse_empty_input() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"");
        assert!(cmds.is_empty());
    }

    // --- C0 control characters ---

    #[test]
    fn parse_newline() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"\n");
        assert_eq!(cmds, vec![TerminalCommand::Newline]);
    }

    #[test]
    fn parse_carriage_return() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"\r");
        assert_eq!(cmds, vec![TerminalCommand::CarriageReturn]);
    }

    #[test]
    fn parse_backspace() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"\x08");
        assert_eq!(cmds, vec![TerminalCommand::Backspace]);
    }

    #[test]
    fn parse_tab() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"\t");
        assert_eq!(cmds, vec![TerminalCommand::Tab]);
    }

    #[test]
    fn parse_bell() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"\x07");
        assert_eq!(cmds, vec![TerminalCommand::Bell]);
    }

    // --- CSI cursor movement ---

    #[test]
    fn parse_cursor_up() {
        let mut parser = VtParser::new();
        // ESC [ 3 A
        let cmds = parser.parse(b"\x1b[3A");
        assert_eq!(cmds, vec![TerminalCommand::CursorUp(3)]);
    }

    #[test]
    fn parse_cursor_up_default() {
        let mut parser = VtParser::new();
        // ESC [ A (no param = default 1)
        let cmds = parser.parse(b"\x1b[A");
        assert_eq!(cmds, vec![TerminalCommand::CursorUp(1)]);
    }

    #[test]
    fn parse_cursor_down() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"\x1b[5B");
        assert_eq!(cmds, vec![TerminalCommand::CursorDown(5)]);
    }

    #[test]
    fn parse_cursor_forward() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"\x1b[2C");
        assert_eq!(cmds, vec![TerminalCommand::CursorForward(2)]);
    }

    #[test]
    fn parse_cursor_back() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"\x1b[4D");
        assert_eq!(cmds, vec![TerminalCommand::CursorBack(4)]);
    }

    #[test]
    fn parse_cursor_position() {
        let mut parser = VtParser::new();
        // ESC [ 10 ; 20 H
        let cmds = parser.parse(b"\x1b[10;20H");
        assert_eq!(cmds, vec![TerminalCommand::CursorPosition { row: 10, col: 20 }]);
    }

    #[test]
    fn parse_cursor_position_default() {
        let mut parser = VtParser::new();
        // ESC [ H (no params = 1;1)
        let cmds = parser.parse(b"\x1b[H");
        assert_eq!(cmds, vec![TerminalCommand::CursorPosition { row: 1, col: 1 }]);
    }

    // --- SGR (Set Graphics Rendition) ---

    #[test]
    fn parse_sgr_reset() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"\x1b[0m");
        assert_eq!(cmds, vec![TerminalCommand::ResetAttributes]);
    }

    #[test]
    fn parse_sgr_reset_no_param() {
        let mut parser = VtParser::new();
        // ESC [ m (no param = reset)
        let cmds = parser.parse(b"\x1b[m");
        assert_eq!(cmds, vec![TerminalCommand::ResetAttributes]);
    }

    #[test]
    fn parse_sgr_bold() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"\x1b[1m");
        assert_eq!(cmds, vec![TerminalCommand::SetBold]);
    }

    #[test]
    fn parse_sgr_dim() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"\x1b[2m");
        assert_eq!(cmds, vec![TerminalCommand::SetDim]);
    }

    #[test]
    fn parse_sgr_italic() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"\x1b[3m");
        assert_eq!(cmds, vec![TerminalCommand::SetItalic]);
    }

    #[test]
    fn parse_sgr_underline() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"\x1b[4m");
        assert_eq!(cmds, vec![TerminalCommand::SetUnderline]);
    }

    #[test]
    fn parse_sgr_inverse() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"\x1b[7m");
        assert_eq!(cmds, vec![TerminalCommand::SetInverse]);
    }

    #[test]
    fn parse_sgr_hidden() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"\x1b[8m");
        assert_eq!(cmds, vec![TerminalCommand::SetHidden]);
    }

    #[test]
    fn parse_sgr_strikethrough() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"\x1b[9m");
        assert_eq!(cmds, vec![TerminalCommand::SetStrikethrough]);
    }

    #[test]
    fn parse_sgr_foreground_basic() {
        let mut parser = VtParser::new();
        // ESC[31m = red foreground (index 1)
        let cmds = parser.parse(b"\x1b[31m");
        assert_eq!(cmds, vec![TerminalCommand::SetForeground(Color::Indexed(1))]);
    }

    #[test]
    fn parse_sgr_background_basic() {
        let mut parser = VtParser::new();
        // ESC[42m = green background (index 2)
        let cmds = parser.parse(b"\x1b[42m");
        assert_eq!(cmds, vec![TerminalCommand::SetBackground(Color::Indexed(2))]);
    }

    #[test]
    fn parse_sgr_foreground_256() {
        let mut parser = VtParser::new();
        // ESC[38;5;196m = 256-color foreground, index 196
        let cmds = parser.parse(b"\x1b[38;5;196m");
        assert_eq!(cmds, vec![TerminalCommand::SetForeground(Color::Indexed(196))]);
    }

    #[test]
    fn parse_sgr_background_256() {
        let mut parser = VtParser::new();
        // ESC[48;5;21m = 256-color background, index 21
        let cmds = parser.parse(b"\x1b[48;5;21m");
        assert_eq!(cmds, vec![TerminalCommand::SetBackground(Color::Indexed(21))]);
    }

    #[test]
    fn parse_sgr_foreground_rgb() {
        let mut parser = VtParser::new();
        // ESC[38;2;255;128;0m = RGB foreground
        let cmds = parser.parse(b"\x1b[38;2;255;128;0m");
        assert_eq!(cmds, vec![
            TerminalCommand::SetForeground(Color::Rgb(Rgb::new(255, 128, 0)))
        ]);
    }

    #[test]
    fn parse_sgr_background_rgb() {
        let mut parser = VtParser::new();
        // ESC[48;2;10;20;30m = RGB background
        let cmds = parser.parse(b"\x1b[48;2;10;20;30m");
        assert_eq!(cmds, vec![
            TerminalCommand::SetBackground(Color::Rgb(Rgb::new(10, 20, 30)))
        ]);
    }

    #[test]
    fn parse_sgr_default_foreground() {
        let mut parser = VtParser::new();
        // ESC[39m = default foreground
        let cmds = parser.parse(b"\x1b[39m");
        assert_eq!(cmds, vec![TerminalCommand::SetForeground(Color::Default)]);
    }

    #[test]
    fn parse_sgr_default_background() {
        let mut parser = VtParser::new();
        // ESC[49m = default background
        let cmds = parser.parse(b"\x1b[49m");
        assert_eq!(cmds, vec![TerminalCommand::SetBackground(Color::Default)]);
    }

    #[test]
    fn parse_sgr_multiple_params() {
        let mut parser = VtParser::new();
        // ESC[1;31m = bold + red foreground
        let cmds = parser.parse(b"\x1b[1;31m");
        assert_eq!(cmds, vec![
            TerminalCommand::SetBold,
            TerminalCommand::SetForeground(Color::Indexed(1)),
        ]);
    }

    #[test]
    fn parse_sgr_bright_foreground() {
        let mut parser = VtParser::new();
        // ESC[91m = bright red foreground (index 9)
        let cmds = parser.parse(b"\x1b[91m");
        assert_eq!(cmds, vec![TerminalCommand::SetForeground(Color::Indexed(9))]);
    }

    #[test]
    fn parse_sgr_bright_background() {
        let mut parser = VtParser::new();
        // ESC[102m = bright green background (index 10)
        let cmds = parser.parse(b"\x1b[102m");
        assert_eq!(cmds, vec![TerminalCommand::SetBackground(Color::Indexed(10))]);
    }

    // --- Erase sequences ---

    #[test]
    fn parse_erase_in_line() {
        let mut parser = VtParser::new();
        // ESC[K = erase to end of line (mode 0)
        let cmds = parser.parse(b"\x1b[K");
        assert_eq!(cmds, vec![TerminalCommand::EraseInLine(0)]);
    }

    #[test]
    fn parse_erase_in_line_mode1() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"\x1b[1K");
        assert_eq!(cmds, vec![TerminalCommand::EraseInLine(1)]);
    }

    #[test]
    fn parse_erase_in_display() {
        let mut parser = VtParser::new();
        // ESC[2J = erase entire display
        let cmds = parser.parse(b"\x1b[2J");
        assert_eq!(cmds, vec![TerminalCommand::EraseInDisplay(2)]);
    }

    // --- Mixed content ---

    #[test]
    fn parse_text_with_newline() {
        let mut parser = VtParser::new();
        let cmds = parser.parse(b"AB\r\nCD");
        assert_eq!(cmds, vec![
            TerminalCommand::Print('A'),
            TerminalCommand::Print('B'),
            TerminalCommand::CarriageReturn,
            TerminalCommand::Newline,
            TerminalCommand::Print('C'),
            TerminalCommand::Print('D'),
        ]);
    }

    #[test]
    fn parse_colored_text() {
        let mut parser = VtParser::new();
        // red "Hi" then reset
        let cmds = parser.parse(b"\x1b[31mHi\x1b[0m");
        assert_eq!(cmds, vec![
            TerminalCommand::SetForeground(Color::Indexed(1)),
            TerminalCommand::Print('H'),
            TerminalCommand::Print('i'),
            TerminalCommand::ResetAttributes,
        ]);
    }

    // --- Partial/split sequences ---

    #[test]
    fn parse_split_escape_sequence() {
        let mut parser = VtParser::new();
        // Split ESC[31m across two chunks
        let cmds1 = parser.parse(b"\x1b[3");
        assert!(cmds1.is_empty(), "partial sequence should produce no commands");

        let cmds2 = parser.parse(b"1m");
        assert_eq!(cmds2, vec![TerminalCommand::SetForeground(Color::Indexed(1))]);
    }

    #[test]
    fn parse_split_text_and_escape() {
        let mut parser = VtParser::new();
        let cmds1 = parser.parse(b"AB\x1b");
        assert_eq!(cmds1, vec![
            TerminalCommand::Print('A'),
            TerminalCommand::Print('B'),
        ]);

        let cmds2 = parser.parse(b"[1m");
        assert_eq!(cmds2, vec![TerminalCommand::SetBold]);
    }

    // --- Unicode ---

    #[test]
    fn parse_unicode_text() {
        let mut parser = VtParser::new();
        let cmds = parser.parse("한글".as_bytes());
        assert_eq!(cmds, vec![
            TerminalCommand::Print('한'),
            TerminalCommand::Print('글'),
        ]);
    }
}
