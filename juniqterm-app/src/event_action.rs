use juniqterm_types::KeyEvent;

#[derive(Debug, PartialEq)]
pub enum Action {
    WritePty(Vec<u8>),
    SetPreedit(String),
}

/// IME Commit → PTY에 보낼 바이트
pub fn handle_ime_commit(text: &str) -> Action {
    Action::WritePty(text.as_bytes().to_vec())
}

/// IME Preedit → preedit 텍스트 저장
pub fn handle_ime_preedit(text: &str) -> Action {
    Action::SetPreedit(text.to_string())
}

/// plain char(수식키 없는 일반 문자)의 KeyboardInput 처리.
/// IME 조합 중(preedit 비어있지 않음)에는 KeyboardInput text를 무시해야
/// 한글 자소 분리가 발생하지 않는다.
/// 조합 중이 아닐 때(영문 입력 등)는 KeyboardInput text를 그대로 PTY에 전달한다.
pub fn handle_plain_char_input(text: Option<&str>, ime_composing: bool) -> Option<Action> {
    if ime_composing {
        return None;
    }
    text.map(|t| Action::WritePty(t.as_bytes().to_vec()))
}

/// 특수키·수식키 → PTY에 보낼 바이트
///
/// 이 함수는 특수키(Enter, Arrow 등)와 수식키(Ctrl+C, Alt+D 등)만 담당한다.
pub fn handle_keyboard_input(key_event: KeyEvent) -> Action {
    Action::WritePty(juniqterm_input::encode(key_event))
}

#[cfg(test)]
mod tests {
    use super::*;
    use juniqterm_types::{Key, Modifiers};

    #[test]
    fn ime_commit_korean() {
        let result = handle_ime_commit("한글");
        assert_eq!(
            result,
            Action::WritePty(vec![0xED, 0x95, 0x9C, 0xEA, 0xB8, 0x80])
        );
    }

    #[test]
    fn ime_commit_ascii() {
        let result = handle_ime_commit("hello");
        assert_eq!(result, Action::WritePty(b"hello".to_vec()));
    }

    #[test]
    fn ime_preedit_composing() {
        let result = handle_ime_preedit("한");
        assert_eq!(result, Action::SetPreedit("한".to_string()));
    }

    #[test]
    fn ime_preedit_empty_ends_composing() {
        let result = handle_ime_preedit("");
        assert_eq!(result, Action::SetPreedit(String::new()));
    }

    // === 특수키·수식키 → PTY ===

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

    // === 첫글자 preedit ===

    #[test]
    fn first_preedit_character_sets_preedit() {
        let result = handle_ime_preedit("ㅎ");
        assert_eq!(result, Action::SetPreedit("ㅎ".to_string()));
    }

    // === plain char + IME 자소 분리 방지 ===

    #[test]
    fn plain_char_suppressed_when_ime_composing() {
        // IME 조합 중(preedit 비어있지 않음)이면 KeyboardInput text 무시
        assert_eq!(handle_plain_char_input(Some("ㅎ"), true), None);
        assert_eq!(handle_plain_char_input(Some("a"), true), None);
    }

    #[test]
    fn plain_char_none_text_when_ime_composing() {
        // IME 조합 중 event.text=None인 경우
        assert_eq!(handle_plain_char_input(None, true), None);
    }

    #[test]
    fn plain_char_sent_when_not_composing() {
        // IME 조합 중이 아닐 때(영문 입력 등) KeyboardInput text 전달
        assert_eq!(
            handle_plain_char_input(Some("a"), false),
            Some(Action::WritePty(b"a".to_vec()))
        );
    }

    #[test]
    fn plain_char_none_text_when_not_composing() {
        assert_eq!(handle_plain_char_input(None, false), None);
    }

    // === 영문 입력 흐름 (IME 비활성) ===

    #[test]
    fn english_input_flow_no_ime() {
        // 영문 입력소스(ABC) 사용 시: IME 없음, preedit 항상 비어있음
        // KeyboardInput만으로 문자 전달
        assert_eq!(
            handle_plain_char_input(Some("a"), false),
            Some(Action::WritePty(b"a".to_vec()))
        );
        assert_eq!(
            handle_plain_char_input(Some("z"), false),
            Some(Action::WritePty(b"z".to_vec()))
        );
        assert_eq!(
            handle_plain_char_input(Some("1"), false),
            Some(Action::WritePty(b"1".to_vec()))
        );
    }

    // === winit 버그: 첫 한글 입력 시 자소 분리 ===

    #[test]
    fn first_korean_key_leaks_jamo_before_ime_enabled() {
        // winit 버그: 첫 한글 입력 시 KeyboardInput이 Ime::Enabled보다 먼저 도착
        // preedit이 비어있어서 자소가 PTY로 직접 전송됨
        //
        // 실제 이벤트 순서:
        // 1) KeyboardInput("ㅇ") ← preedit="", ime_composing=false
        // 2) Ime::Enabled
        // 3) Ime::Preedit("ㅏ") ...
        let result = handle_plain_char_input(Some("ㅇ"), false);
        // 현재: 자소 "ㅇ"이 PTY로 전송됨 (버그)
        assert_eq!(result, Some(Action::WritePty("ㅇ".as_bytes().to_vec())));
    }

    // === 한글 입력 전체 흐름 시뮬레이션 ===

    #[test]
    fn korean_input_flow_no_jamo_leak() {
        // "한" 입력 시뮬레이션: ㅎ → ㅏ → ㄴ
        // 1) 첫 키 "ㅎ": macOS에서 Ime::Preedit이 KeyboardInput보다 먼저 오므로
        //    preedit이 비어있지 않아 KeyboardInput은 무시됨
        assert_eq!(handle_plain_char_input(Some("ㅎ"), true), None);

        // 2) IME 활성화 후 preedit 진행
        assert_eq!(handle_ime_preedit("ㅎ"), Action::SetPreedit("ㅎ".into()));
        assert_eq!(handle_ime_preedit("하"), Action::SetPreedit("하".into()));
        assert_eq!(handle_ime_preedit("한"), Action::SetPreedit("한".into()));

        // 3) 조합 완료 → commit
        assert_eq!(handle_ime_preedit(""), Action::SetPreedit(String::new()));
        assert_eq!(
            handle_ime_commit("한"),
            Action::WritePty("한".as_bytes().to_vec())
        );
    }
}
