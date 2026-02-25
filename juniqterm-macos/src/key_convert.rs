use crate::event::Modifiers;

/// macOS 가상 키코드 (Carbon kVK_ 상수)
pub mod keycode {
    pub const RETURN: u16 = 0x24;
    pub const TAB: u16 = 0x30;
    pub const SPACE: u16 = 0x31;
    pub const DELETE: u16 = 0x33; // Backspace
    pub const ESCAPE: u16 = 0x35;
    pub const FORWARD_DELETE: u16 = 0x75;
    pub const UP_ARROW: u16 = 0x7E;
    pub const DOWN_ARROW: u16 = 0x7D;
    pub const LEFT_ARROW: u16 = 0x7B;
    pub const RIGHT_ARROW: u16 = 0x7C;
    pub const HOME: u16 = 0x73;
    pub const END: u16 = 0x77;
    pub const PAGE_UP: u16 = 0x74;
    pub const PAGE_DOWN: u16 = 0x79;
}

/// macOS keycode + characters → juniqterm_types::KeyEvent 변환
pub fn convert_key(
    keycode: u16,
    characters: Option<&str>,
    modifiers: Modifiers,
) -> Option<juniqterm_types::KeyEvent> {
    let mods = convert_modifiers(modifiers);

    let key = match keycode {
        keycode::RETURN => juniqterm_types::Key::Enter,
        keycode::TAB => juniqterm_types::Key::Tab,
        keycode::ESCAPE => juniqterm_types::Key::Escape,
        keycode::DELETE => juniqterm_types::Key::Backspace,
        keycode::FORWARD_DELETE => juniqterm_types::Key::Delete,
        keycode::UP_ARROW => juniqterm_types::Key::ArrowUp,
        keycode::DOWN_ARROW => juniqterm_types::Key::ArrowDown,
        keycode::LEFT_ARROW => juniqterm_types::Key::ArrowLeft,
        keycode::RIGHT_ARROW => juniqterm_types::Key::ArrowRight,
        keycode::HOME => juniqterm_types::Key::Home,
        keycode::END => juniqterm_types::Key::End,
        keycode::PAGE_UP => juniqterm_types::Key::PageUp,
        keycode::PAGE_DOWN => juniqterm_types::Key::PageDown,
        keycode::SPACE => juniqterm_types::Key::Char(' '),
        _ => {
            // 문자 키: characters에서 추출
            let c = characters.and_then(|s| {
                let mut chars = s.chars();
                let c = chars.next()?;
                if chars.next().is_some() {
                    return None;
                }
                Some(c)
            })?;
            juniqterm_types::Key::Char(c)
        }
    };

    Some(juniqterm_types::KeyEvent {
        key,
        modifiers: mods,
    })
}

fn convert_modifiers(modifiers: Modifiers) -> juniqterm_types::Modifiers {
    let mut mods = juniqterm_types::Modifiers::empty();
    if modifiers.contains(Modifiers::CONTROL) {
        mods |= juniqterm_types::Modifiers::CTRL;
    }
    if modifiers.contains(Modifiers::ALT) {
        mods |= juniqterm_types::Modifiers::ALT;
    }
    if modifiers.contains(Modifiers::SHIFT) {
        mods |= juniqterm_types::Modifiers::SHIFT;
    }
    mods
}

#[cfg(test)]
mod tests {
    use super::*;
    use juniqterm_types::{Key, KeyEvent, Modifiers as TypeMods};

    #[test]
    fn return_key() {
        let result = convert_key(keycode::RETURN, None, Modifiers::empty());
        assert_eq!(
            result,
            Some(KeyEvent { key: Key::Enter, modifiers: TypeMods::empty() })
        );
    }

    #[test]
    fn tab_key() {
        let result = convert_key(keycode::TAB, None, Modifiers::empty());
        assert_eq!(
            result,
            Some(KeyEvent { key: Key::Tab, modifiers: TypeMods::empty() })
        );
    }

    #[test]
    fn escape_key() {
        let result = convert_key(keycode::ESCAPE, None, Modifiers::empty());
        assert_eq!(
            result,
            Some(KeyEvent { key: Key::Escape, modifiers: TypeMods::empty() })
        );
    }

    #[test]
    fn backspace_key() {
        let result = convert_key(keycode::DELETE, None, Modifiers::empty());
        assert_eq!(
            result,
            Some(KeyEvent { key: Key::Backspace, modifiers: TypeMods::empty() })
        );
    }

    #[test]
    fn forward_delete_key() {
        let result = convert_key(keycode::FORWARD_DELETE, None, Modifiers::empty());
        assert_eq!(
            result,
            Some(KeyEvent { key: Key::Delete, modifiers: TypeMods::empty() })
        );
    }

    #[test]
    fn arrow_keys() {
        assert_eq!(
            convert_key(keycode::UP_ARROW, None, Modifiers::empty()).unwrap().key,
            Key::ArrowUp
        );
        assert_eq!(
            convert_key(keycode::DOWN_ARROW, None, Modifiers::empty()).unwrap().key,
            Key::ArrowDown
        );
        assert_eq!(
            convert_key(keycode::LEFT_ARROW, None, Modifiers::empty()).unwrap().key,
            Key::ArrowLeft
        );
        assert_eq!(
            convert_key(keycode::RIGHT_ARROW, None, Modifiers::empty()).unwrap().key,
            Key::ArrowRight
        );
    }

    #[test]
    fn home_end_page_keys() {
        assert_eq!(
            convert_key(keycode::HOME, None, Modifiers::empty()).unwrap().key,
            Key::Home
        );
        assert_eq!(
            convert_key(keycode::END, None, Modifiers::empty()).unwrap().key,
            Key::End
        );
        assert_eq!(
            convert_key(keycode::PAGE_UP, None, Modifiers::empty()).unwrap().key,
            Key::PageUp
        );
        assert_eq!(
            convert_key(keycode::PAGE_DOWN, None, Modifiers::empty()).unwrap().key,
            Key::PageDown
        );
    }

    #[test]
    fn space_key() {
        let result = convert_key(keycode::SPACE, None, Modifiers::empty());
        assert_eq!(
            result,
            Some(KeyEvent { key: Key::Char(' '), modifiers: TypeMods::empty() })
        );
    }

    #[test]
    fn character_key() {
        let result = convert_key(0x00, Some("a"), Modifiers::empty()); // 0x00 = kVK_ANSI_A
        assert_eq!(
            result,
            Some(KeyEvent { key: Key::Char('a'), modifiers: TypeMods::empty() })
        );
    }

    #[test]
    fn ctrl_c() {
        let result = convert_key(0x08, Some("c"), Modifiers::CONTROL); // 0x08 = kVK_ANSI_C
        assert_eq!(
            result,
            Some(KeyEvent { key: Key::Char('c'), modifiers: TypeMods::CTRL })
        );
    }

    #[test]
    fn alt_d() {
        let result = convert_key(0x02, Some("d"), Modifiers::ALT); // 0x02 = kVK_ANSI_D
        assert_eq!(
            result,
            Some(KeyEvent { key: Key::Char('d'), modifiers: TypeMods::ALT })
        );
    }

    #[test]
    fn shift_arrow() {
        let result = convert_key(keycode::UP_ARROW, None, Modifiers::SHIFT | Modifiers::CONTROL);
        let result = result.unwrap();
        assert_eq!(result.key, Key::ArrowUp);
        assert!(result.modifiers.contains(TypeMods::CTRL));
        assert!(result.modifiers.contains(TypeMods::SHIFT));
    }

    #[test]
    fn unknown_keycode_no_characters_returns_none() {
        let result = convert_key(0xFF, None, Modifiers::empty());
        assert_eq!(result, None);
    }

    #[test]
    fn multi_char_characters_returns_none() {
        let result = convert_key(0x00, Some("ab"), Modifiers::empty());
        assert_eq!(result, None);
    }
}
