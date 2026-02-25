use growterm_types::{Key, KeyEvent, Modifiers};

/// Convert a KeyEvent to the byte sequence a terminal PTY expects.
pub fn encode(event: KeyEvent) -> Vec<u8> {
    let has_alt = event.modifiers.contains(Modifiers::ALT);
    let has_ctrl = event.modifiers.contains(Modifiers::CTRL);
    let has_shift = event.modifiers.contains(Modifiers::SHIFT);

    match event.key {
        Key::Char(c) if has_ctrl && c.is_ascii_alphabetic() => {
            // Ctrl+A = 0x01, Ctrl+Z = 0x1A
            let ctrl_byte = (c.to_ascii_lowercase() as u8) - b'a' + 1;
            if has_alt {
                vec![0x1b, ctrl_byte]
            } else {
                vec![ctrl_byte]
            }
        }
        Key::Char(c) => {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            if has_alt {
                let mut v = vec![0x1b];
                v.extend_from_slice(s.as_bytes());
                v
            } else {
                s.as_bytes().to_vec()
            }
        }
        Key::Enter => vec![b'\r'],
        Key::Tab => vec![b'\t'],
        Key::Escape => vec![0x1b],
        Key::Backspace => vec![0x7f],
        Key::Delete => encode_tilde(3, has_shift, has_alt, has_ctrl),
        Key::ArrowUp => encode_cursor(b'A', has_shift, has_alt, has_ctrl),
        Key::ArrowDown => encode_cursor(b'B', has_shift, has_alt, has_ctrl),
        Key::ArrowRight => encode_cursor(b'C', has_shift, has_alt, has_ctrl),
        Key::ArrowLeft => encode_cursor(b'D', has_shift, has_alt, has_ctrl),
        Key::Home => encode_cursor(b'H', has_shift, has_alt, has_ctrl),
        Key::End => encode_cursor(b'F', has_shift, has_alt, has_ctrl),
        Key::PageUp => encode_tilde(5, has_shift, has_alt, has_ctrl),
        Key::PageDown => encode_tilde(6, has_shift, has_alt, has_ctrl),
    }
}

/// Modifier parameter for xterm-style sequences: CSI 1;{mod} {letter}
fn modifier_param(shift: bool, alt: bool, ctrl: bool) -> Option<u8> {
    let n = 1 + (shift as u8) + (alt as u8) * 2 + (ctrl as u8) * 4;
    if n > 1 { Some(n) } else { None }
}

/// Encode cursor-key style sequences: \x1b[A or \x1b[1;{mod}A
fn encode_cursor(letter: u8, shift: bool, alt: bool, ctrl: bool) -> Vec<u8> {
    match modifier_param(shift, alt, ctrl) {
        Some(m) => {
            let mut v = vec![0x1b, b'[', b'1', b';'];
            v.push(b'0' + m);
            v.push(letter);
            v
        }
        None => vec![0x1b, b'[', letter],
    }
}

/// Encode tilde-style sequences: \x1b[{n}~ or \x1b[{n};{mod}~
fn encode_tilde(n: u8, shift: bool, alt: bool, ctrl: bool) -> Vec<u8> {
    match modifier_param(shift, alt, ctrl) {
        Some(m) => {
            let mut v = vec![0x1b, b'['];
            v.push(b'0' + n);
            v.push(b';');
            v.push(b'0' + m);
            v.push(b'~');
            v
        }
        None => vec![0x1b, b'[', b'0' + n, b'~'],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Plain characters ---

    #[test]
    fn ascii_char() {
        let event = KeyEvent { key: Key::Char('a'), modifiers: Modifiers::empty() };
        assert_eq!(encode(event), b"a");
    }

    #[test]
    fn uppercase_char() {
        let event = KeyEvent { key: Key::Char('A'), modifiers: Modifiers::empty() };
        assert_eq!(encode(event), b"A");
    }

    #[test]
    fn unicode_char() {
        let event = KeyEvent { key: Key::Char('한'), modifiers: Modifiers::empty() };
        assert_eq!(encode(event), "한".as_bytes());
    }

    #[test]
    fn space() {
        let event = KeyEvent { key: Key::Char(' '), modifiers: Modifiers::empty() };
        assert_eq!(encode(event), b" ");
    }

    // --- Special keys ---

    #[test]
    fn enter() {
        let event = KeyEvent { key: Key::Enter, modifiers: Modifiers::empty() };
        assert_eq!(encode(event), b"\r");
    }

    #[test]
    fn tab() {
        let event = KeyEvent { key: Key::Tab, modifiers: Modifiers::empty() };
        assert_eq!(encode(event), b"\t");
    }

    #[test]
    fn escape() {
        let event = KeyEvent { key: Key::Escape, modifiers: Modifiers::empty() };
        assert_eq!(encode(event), b"\x1b");
    }

    #[test]
    fn backspace() {
        let event = KeyEvent { key: Key::Backspace, modifiers: Modifiers::empty() };
        assert_eq!(encode(event), b"\x7f");
    }

    #[test]
    fn delete() {
        let event = KeyEvent { key: Key::Delete, modifiers: Modifiers::empty() };
        assert_eq!(encode(event), b"\x1b[3~");
    }

    // --- Arrow keys ---

    #[test]
    fn arrow_up() {
        let event = KeyEvent { key: Key::ArrowUp, modifiers: Modifiers::empty() };
        assert_eq!(encode(event), b"\x1b[A");
    }

    #[test]
    fn arrow_down() {
        let event = KeyEvent { key: Key::ArrowDown, modifiers: Modifiers::empty() };
        assert_eq!(encode(event), b"\x1b[B");
    }

    #[test]
    fn arrow_right() {
        let event = KeyEvent { key: Key::ArrowRight, modifiers: Modifiers::empty() };
        assert_eq!(encode(event), b"\x1b[C");
    }

    #[test]
    fn arrow_left() {
        let event = KeyEvent { key: Key::ArrowLeft, modifiers: Modifiers::empty() };
        assert_eq!(encode(event), b"\x1b[D");
    }

    // --- Navigation keys ---

    #[test]
    fn home() {
        let event = KeyEvent { key: Key::Home, modifiers: Modifiers::empty() };
        assert_eq!(encode(event), b"\x1b[H");
    }

    #[test]
    fn end() {
        let event = KeyEvent { key: Key::End, modifiers: Modifiers::empty() };
        assert_eq!(encode(event), b"\x1b[F");
    }

    #[test]
    fn page_up() {
        let event = KeyEvent { key: Key::PageUp, modifiers: Modifiers::empty() };
        assert_eq!(encode(event), b"\x1b[5~");
    }

    #[test]
    fn page_down() {
        let event = KeyEvent { key: Key::PageDown, modifiers: Modifiers::empty() };
        assert_eq!(encode(event), b"\x1b[6~");
    }

    // --- Ctrl + character ---

    #[test]
    fn ctrl_a() {
        let event = KeyEvent { key: Key::Char('a'), modifiers: Modifiers::CTRL };
        assert_eq!(encode(event), b"\x01");
    }

    #[test]
    fn ctrl_c() {
        let event = KeyEvent { key: Key::Char('c'), modifiers: Modifiers::CTRL };
        assert_eq!(encode(event), b"\x03");
    }

    #[test]
    fn ctrl_z() {
        let event = KeyEvent { key: Key::Char('z'), modifiers: Modifiers::CTRL };
        assert_eq!(encode(event), b"\x1a");
    }

    #[test]
    fn ctrl_uppercase_a() {
        // Ctrl+A and Ctrl+Shift+A should both produce 0x01
        let event = KeyEvent { key: Key::Char('A'), modifiers: Modifiers::CTRL };
        assert_eq!(encode(event), b"\x01");
    }

    // --- Alt + character ---

    #[test]
    fn alt_a() {
        let event = KeyEvent { key: Key::Char('a'), modifiers: Modifiers::ALT };
        assert_eq!(encode(event), b"\x1ba");
    }

    #[test]
    fn alt_uppercase() {
        let event = KeyEvent { key: Key::Char('A'), modifiers: Modifiers::ALT };
        assert_eq!(encode(event), b"\x1bA");
    }

    // --- Alt + special keys ---

    #[test]
    fn alt_arrow_up() {
        let event = KeyEvent { key: Key::ArrowUp, modifiers: Modifiers::ALT };
        assert_eq!(encode(event), b"\x1b[1;3A");
    }

    #[test]
    fn alt_arrow_down() {
        let event = KeyEvent { key: Key::ArrowDown, modifiers: Modifiers::ALT };
        assert_eq!(encode(event), b"\x1b[1;3B");
    }

    // --- Shift + arrow (modified cursor keys) ---

    #[test]
    fn shift_arrow_up() {
        let event = KeyEvent { key: Key::ArrowUp, modifiers: Modifiers::SHIFT };
        assert_eq!(encode(event), b"\x1b[1;2A");
    }

    #[test]
    fn shift_arrow_right() {
        let event = KeyEvent { key: Key::ArrowRight, modifiers: Modifiers::SHIFT };
        assert_eq!(encode(event), b"\x1b[1;2C");
    }

    // --- Ctrl + arrow ---

    #[test]
    fn ctrl_arrow_left() {
        let event = KeyEvent { key: Key::ArrowLeft, modifiers: Modifiers::CTRL };
        assert_eq!(encode(event), b"\x1b[1;5D");
    }

    #[test]
    fn ctrl_arrow_right() {
        let event = KeyEvent { key: Key::ArrowRight, modifiers: Modifiers::CTRL };
        assert_eq!(encode(event), b"\x1b[1;5C");
    }

    // --- Ctrl+Alt combination ---

    #[test]
    fn ctrl_alt_arrow_up() {
        let event = KeyEvent { key: Key::ArrowUp, modifiers: Modifiers::CTRL | Modifiers::ALT };
        assert_eq!(encode(event), b"\x1b[1;7A");
    }

    // --- Shift char (no special encoding) ---

    #[test]
    fn shift_char_is_just_the_char() {
        // Shift is already reflected in the char value (e.g. 'A' instead of 'a')
        let event = KeyEvent { key: Key::Char('A'), modifiers: Modifiers::SHIFT };
        assert_eq!(encode(event), b"A");
    }

    // --- Edge: Ctrl + non-alpha ---

    #[test]
    fn ctrl_non_alpha_ignored() {
        // Ctrl+1 has no standard encoding → send '1' as-is
        let event = KeyEvent { key: Key::Char('1'), modifiers: Modifiers::CTRL };
        assert_eq!(encode(event), b"1");
    }
}
