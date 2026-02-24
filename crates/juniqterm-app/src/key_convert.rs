use juniqterm_types::{Key, KeyEvent, Modifiers};
use winit::event::ElementState;
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey};

pub fn convert_key(
    logical_key: &WinitKey,
    state: ElementState,
    modifiers: ModifiersState,
) -> Option<KeyEvent> {
    if state != ElementState::Pressed {
        return None;
    }

    let mods = convert_modifiers(modifiers);

    let key = match logical_key {
        WinitKey::Named(named) => match named {
            NamedKey::Enter => Key::Enter,
            NamedKey::Tab => Key::Tab,
            NamedKey::Escape => Key::Escape,
            NamedKey::Backspace => Key::Backspace,
            NamedKey::Delete => Key::Delete,
            NamedKey::ArrowUp => Key::ArrowUp,
            NamedKey::ArrowDown => Key::ArrowDown,
            NamedKey::ArrowLeft => Key::ArrowLeft,
            NamedKey::ArrowRight => Key::ArrowRight,
            NamedKey::Home => Key::Home,
            NamedKey::End => Key::End,
            NamedKey::PageUp => Key::PageUp,
            NamedKey::PageDown => Key::PageDown,
            NamedKey::Space => Key::Char(' '),
            _ => return None,
        },
        WinitKey::Character(s) => {
            let mut chars = s.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            Key::Char(c)
        }
        _ => return None,
    };

    Some(KeyEvent {
        key,
        modifiers: mods,
    })
}

fn convert_modifiers(modifiers: ModifiersState) -> Modifiers {
    let mut mods = Modifiers::empty();
    if modifiers.control_key() {
        mods |= Modifiers::CTRL;
    }
    if modifiers.alt_key() {
        mods |= Modifiers::ALT;
    }
    if modifiers.shift_key() {
        mods |= Modifiers::SHIFT;
    }
    mods
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pressed(key: WinitKey, mods: ModifiersState) -> Option<KeyEvent> {
        convert_key(&key, ElementState::Pressed, mods)
    }

    fn no_mods() -> ModifiersState {
        ModifiersState::empty()
    }

    // --- Released keys produce None ---

    #[test]
    fn released_key_returns_none() {
        let result = convert_key(
            &WinitKey::Character("a".into()),
            ElementState::Released,
            no_mods(),
        );
        assert_eq!(result, None);
    }

    // --- Simple character keys ---

    #[test]
    fn simple_char_a() {
        assert_eq!(
            pressed(WinitKey::Character("a".into()), no_mods()),
            Some(KeyEvent {
                key: Key::Char('a'),
                modifiers: Modifiers::empty(),
            })
        );
    }

    #[test]
    fn simple_char_uppercase() {
        assert_eq!(
            pressed(WinitKey::Character("A".into()), ModifiersState::SHIFT),
            Some(KeyEvent {
                key: Key::Char('A'),
                modifiers: Modifiers::SHIFT,
            })
        );
    }

    #[test]
    fn simple_char_digit() {
        assert_eq!(
            pressed(WinitKey::Character("5".into()), no_mods()),
            Some(KeyEvent {
                key: Key::Char('5'),
                modifiers: Modifiers::empty(),
            })
        );
    }

    #[test]
    fn simple_char_symbol() {
        assert_eq!(
            pressed(WinitKey::Character("/".into()), no_mods()),
            Some(KeyEvent {
                key: Key::Char('/'),
                modifiers: Modifiers::empty(),
            })
        );
    }

    #[test]
    fn unicode_char() {
        assert_eq!(
            pressed(WinitKey::Character("한".into()), no_mods()),
            Some(KeyEvent {
                key: Key::Char('한'),
                modifiers: Modifiers::empty(),
            })
        );
    }

    #[test]
    fn multi_char_string_returns_none() {
        assert_eq!(
            pressed(WinitKey::Character("ab".into()), no_mods()),
            None
        );
    }

    // --- Named keys ---

    #[test]
    fn enter_key() {
        assert_eq!(
            pressed(WinitKey::Named(NamedKey::Enter), no_mods()),
            Some(KeyEvent {
                key: Key::Enter,
                modifiers: Modifiers::empty(),
            })
        );
    }

    #[test]
    fn tab_key() {
        assert_eq!(
            pressed(WinitKey::Named(NamedKey::Tab), no_mods()),
            Some(KeyEvent {
                key: Key::Tab,
                modifiers: Modifiers::empty(),
            })
        );
    }

    #[test]
    fn escape_key() {
        assert_eq!(
            pressed(WinitKey::Named(NamedKey::Escape), no_mods()),
            Some(KeyEvent {
                key: Key::Escape,
                modifiers: Modifiers::empty(),
            })
        );
    }

    #[test]
    fn backspace_key() {
        assert_eq!(
            pressed(WinitKey::Named(NamedKey::Backspace), no_mods()),
            Some(KeyEvent {
                key: Key::Backspace,
                modifiers: Modifiers::empty(),
            })
        );
    }

    #[test]
    fn delete_key() {
        assert_eq!(
            pressed(WinitKey::Named(NamedKey::Delete), no_mods()),
            Some(KeyEvent {
                key: Key::Delete,
                modifiers: Modifiers::empty(),
            })
        );
    }

    #[test]
    fn arrow_keys() {
        assert_eq!(
            pressed(WinitKey::Named(NamedKey::ArrowUp), no_mods()).unwrap().key,
            Key::ArrowUp
        );
        assert_eq!(
            pressed(WinitKey::Named(NamedKey::ArrowDown), no_mods()).unwrap().key,
            Key::ArrowDown
        );
        assert_eq!(
            pressed(WinitKey::Named(NamedKey::ArrowLeft), no_mods()).unwrap().key,
            Key::ArrowLeft
        );
        assert_eq!(
            pressed(WinitKey::Named(NamedKey::ArrowRight), no_mods()).unwrap().key,
            Key::ArrowRight
        );
    }

    #[test]
    fn home_end_page_keys() {
        assert_eq!(
            pressed(WinitKey::Named(NamedKey::Home), no_mods()).unwrap().key,
            Key::Home
        );
        assert_eq!(
            pressed(WinitKey::Named(NamedKey::End), no_mods()).unwrap().key,
            Key::End
        );
        assert_eq!(
            pressed(WinitKey::Named(NamedKey::PageUp), no_mods()).unwrap().key,
            Key::PageUp
        );
        assert_eq!(
            pressed(WinitKey::Named(NamedKey::PageDown), no_mods()).unwrap().key,
            Key::PageDown
        );
    }

    #[test]
    fn space_key() {
        assert_eq!(
            pressed(WinitKey::Named(NamedKey::Space), no_mods()),
            Some(KeyEvent {
                key: Key::Char(' '),
                modifiers: Modifiers::empty(),
            })
        );
    }

    // --- Modifier combinations ---

    #[test]
    fn ctrl_c() {
        assert_eq!(
            pressed(WinitKey::Character("c".into()), ModifiersState::CONTROL),
            Some(KeyEvent {
                key: Key::Char('c'),
                modifiers: Modifiers::CTRL,
            })
        );
    }

    #[test]
    fn alt_d() {
        assert_eq!(
            pressed(WinitKey::Character("d".into()), ModifiersState::ALT),
            Some(KeyEvent {
                key: Key::Char('d'),
                modifiers: Modifiers::ALT,
            })
        );
    }

    #[test]
    fn ctrl_shift_arrow() {
        let mods = ModifiersState::CONTROL | ModifiersState::SHIFT;
        let result = pressed(WinitKey::Named(NamedKey::ArrowUp), mods).unwrap();
        assert_eq!(result.key, Key::ArrowUp);
        assert!(result.modifiers.contains(Modifiers::CTRL));
        assert!(result.modifiers.contains(Modifiers::SHIFT));
    }

    // --- Unsupported keys return None ---

    #[test]
    fn unsupported_named_key_returns_none() {
        assert_eq!(
            pressed(WinitKey::Named(NamedKey::F1), no_mods()),
            None
        );
    }

    #[test]
    fn dead_key_returns_none() {
        assert_eq!(
            pressed(WinitKey::Dead(Some('`')), no_mods()),
            None
        );
    }
}
