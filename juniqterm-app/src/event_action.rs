use juniqterm_types::KeyEvent;

#[derive(Debug, PartialEq)]
pub enum Action {
    WritePty(Vec<u8>),
}

/// 특수키·수식키 → PTY에 보낼 바이트
pub fn handle_keyboard_input(key_event: KeyEvent) -> Action {
    Action::WritePty(juniqterm_input::encode(key_event))
}

#[cfg(test)]
mod tests {
    use super::*;
    use juniqterm_types::{Key, Modifiers};

    #[test]
    fn ctrl_c_writes_pty() {
        let result = handle_keyboard_input(KeyEvent {
            key: Key::Char('c'),
            modifiers: Modifiers::CTRL,
        });
        assert_eq!(result, Action::WritePty(vec![0x03]));
    }

    #[test]
    fn enter_writes_pty() {
        let result = handle_keyboard_input(KeyEvent {
            key: Key::Enter,
            modifiers: Modifiers::empty(),
        });
        assert_eq!(result, Action::WritePty(vec![0x0D]));
    }

    #[test]
    fn alt_character_writes_pty() {
        let result = handle_keyboard_input(KeyEvent {
            key: Key::Char('d'),
            modifiers: Modifiers::ALT,
        });
        assert_eq!(result, Action::WritePty(vec![0x1b, b'd']));
    }

    #[test]
    fn arrow_key_writes_pty() {
        let result = handle_keyboard_input(KeyEvent {
            key: Key::ArrowUp,
            modifiers: Modifiers::empty(),
        });
        assert_eq!(result, Action::WritePty(vec![0x1b, b'[', b'A']));
    }
}
