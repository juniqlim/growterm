use juniqterm_types::KeyEvent;

#[derive(Debug, PartialEq)]
pub enum Action {
    WritePty(Vec<u8>),
    SetPreedit(String),
}

/// 한글 IME 보정 상태 관리.
/// winit 버그: 첫 한글 입력 시 KeyboardInput이 Ime::Enabled보다 먼저 도착하여
/// 초성이 누락되는 문제를 보정한다.
pub struct ImeHandler {
    pending_jamo: Option<char>,
    /// 누락된 초성으로 인해 깨진 자모들을 모아 직접 조합하는 버퍼
    composing_buf: Vec<char>,
    preedit: String,
}

impl ImeHandler {
    pub fn new() -> Self {
        Self {
            pending_jamo: None,
            composing_buf: Vec::new(),
            preedit: String::new(),
        }
    }

    pub fn preedit(&self) -> &str {
        &self.preedit
    }

    pub fn is_composing(&self) -> bool {
        !self.preedit.is_empty()
    }

    /// plain char KeyboardInput 처리.
    /// 한글 자모이면 pending에 저장하고 무시, 영문이면 PTY에 전달.
    pub fn handle_plain_char_input(&mut self, text: Option<&str>) -> Option<Action> {
        if self.is_composing() {
            return None;
        }
        let t = text?;
        if let Some(c) = single_jamo(t) {
            self.pending_jamo = Some(c);
            self.composing_buf.clear();
            self.composing_buf.push(c);
            return None;
        }
        Some(Action::WritePty(t.as_bytes().to_vec()))
    }

    /// IME Preedit 처리.
    /// pending_jamo가 있고 preedit이 단독 중성이면 초성+중성을 조합한다.
    pub fn handle_ime_preedit(&mut self, text: &str) -> Action {
        if let Some(cho) = self.pending_jamo.take() {
            if let Some(jung) = single_jamo(text) {
                if let Some(composed) = compose_chojung(cho, jung) {
                    self.preedit = composed.to_string();
                    return Action::SetPreedit(self.preedit.clone());
                }
            }
        }
        self.preedit = text.to_string();
        Action::SetPreedit(self.preedit.clone())
    }

    /// IME Commit 처리.
    /// 단독 자모 commit은 composing_buf에 모아서 직접 조합한다.
    pub fn handle_ime_commit(&mut self, text: &str) -> Action {
        self.preedit.clear();

        if let Some(jamo) = single_jamo(text) {
            if !self.composing_buf.is_empty() {
                self.composing_buf.push(jamo);

                // 초성+중성+종성 완성 → 바로 전송
                if self.composing_buf.len() == 3 {
                    if let Some(syllable) = try_compose(&self.composing_buf) {
                        self.composing_buf.clear();
                        return Action::WritePty(syllable.to_string().as_bytes().to_vec());
                    }
                }

                // 초성+중성만 있음 → 종성 올 수 있으니 대기
                return Action::SetPreedit(String::new());
            }
        }

        // 정상 commit (음절 단위) — 먼저 composing_buf 잔여분 flush
        let mut bytes = Vec::new();
        if !self.composing_buf.is_empty() {
            if let Some(syllable) = try_compose(&self.composing_buf) {
                bytes.extend(syllable.to_string().as_bytes());
            }
            self.composing_buf.clear();
        }
        bytes.extend(text.as_bytes());
        Action::WritePty(bytes)
    }
}

/// 특수키·수식키 → PTY에 보낼 바이트
pub fn handle_keyboard_input(key_event: KeyEvent) -> Action {
    Action::WritePty(juniqterm_input::encode(key_event))
}

fn single_jamo(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    if matches!(c,
        '\u{1100}'..='\u{11FF}'
        | '\u{3131}'..='\u{318E}'
    ) {
        Some(c)
    } else {
        None
    }
}



/// composing_buf의 자모들을 초성+중성(+종성)으로 조합 시도.
fn try_compose(buf: &[char]) -> Option<char> {
    match buf.len() {
        2 => {
            // 초성 + 중성
            compose_chojung(buf[0], buf[1])
        }
        3 => {
            // 초성 + 중성 + 종성
            let base = compose_chojung(buf[0], buf[1])?;
            let jong_idx = jong_index(buf[2])?;
            char::from_u32(base as u32 + jong_idx)
        }
        _ => None,
    }
}

fn jong_index(c: char) -> Option<u32> {
    match c {
        'ㄱ' => Some(1), 'ㄲ' => Some(2), 'ㄳ' => Some(3), 'ㄴ' => Some(4),
        'ㄵ' => Some(5), 'ㄶ' => Some(6), 'ㄷ' => Some(7), 'ㄹ' => Some(8),
        'ㄺ' => Some(9), 'ㄻ' => Some(10), 'ㄼ' => Some(11), 'ㄽ' => Some(12),
        'ㄾ' => Some(13), 'ㄿ' => Some(14), 'ㅀ' => Some(15), 'ㅁ' => Some(16),
        'ㅂ' => Some(17), 'ㅄ' => Some(18), 'ㅅ' => Some(19), 'ㅆ' => Some(20),
        'ㅇ' => Some(21), 'ㅈ' => Some(22), 'ㅊ' => Some(23), 'ㅋ' => Some(24),
        'ㅌ' => Some(25), 'ㅍ' => Some(26), 'ㅎ' => Some(27),
        _ => None,
    }
}

fn compose_chojung(cho: char, jung: char) -> Option<char> {
    let cho_idx = match cho {
        'ㄱ' => 0, 'ㄲ' => 1, 'ㄴ' => 2, 'ㄷ' => 3, 'ㄸ' => 4,
        'ㄹ' => 5, 'ㅁ' => 6, 'ㅂ' => 7, 'ㅃ' => 8, 'ㅅ' => 9,
        'ㅆ' => 10, 'ㅇ' => 11, 'ㅈ' => 12, 'ㅉ' => 13, 'ㅊ' => 14,
        'ㅋ' => 15, 'ㅌ' => 16, 'ㅍ' => 17, 'ㅎ' => 18,
        _ => return None,
    };
    let jung_idx = match jung {
        'ㅏ' => 0, 'ㅐ' => 1, 'ㅑ' => 2, 'ㅒ' => 3, 'ㅓ' => 4,
        'ㅔ' => 5, 'ㅕ' => 6, 'ㅖ' => 7, 'ㅗ' => 8, 'ㅘ' => 9,
        'ㅙ' => 10, 'ㅚ' => 11, 'ㅛ' => 12, 'ㅜ' => 13, 'ㅝ' => 14,
        'ㅞ' => 15, 'ㅟ' => 16, 'ㅠ' => 17, 'ㅡ' => 18, 'ㅢ' => 19,
        'ㅣ' => 20,
        _ => return None,
    };
    let code = 0xAC00u32 + cho_idx * 21 * 28 + jung_idx * 28;
    char::from_u32(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use juniqterm_types::{Key, Modifiers};

    #[test]
    fn ime_commit_korean() {
        let mut h = ImeHandler::new();
        let result = h.handle_ime_commit("한글");
        assert_eq!(
            result,
            Action::WritePty(vec![0xED, 0x95, 0x9C, 0xEA, 0xB8, 0x80])
        );
    }

    #[test]
    fn ime_commit_ascii() {
        let mut h = ImeHandler::new();
        let result = h.handle_ime_commit("hello");
        assert_eq!(result, Action::WritePty(b"hello".to_vec()));
    }

    #[test]
    fn ime_preedit_composing() {
        let mut h = ImeHandler::new();
        let result = h.handle_ime_preedit("한");
        assert_eq!(result, Action::SetPreedit("한".to_string()));
    }

    #[test]
    fn ime_preedit_empty_ends_composing() {
        let mut h = ImeHandler::new();
        let result = h.handle_ime_preedit("");
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
        let mut h = ImeHandler::new();
        let result = h.handle_ime_preedit("ㅎ");
        assert_eq!(result, Action::SetPreedit("ㅎ".to_string()));
    }

    // === plain char + IME 자소 분리 방지 ===

    #[test]
    fn plain_char_suppressed_when_ime_composing() {
        let mut h = ImeHandler::new();
        h.handle_ime_preedit("ㅎ"); // preedit 활성화
        assert_eq!(h.handle_plain_char_input(Some("ㅎ")), None);
        assert_eq!(h.handle_plain_char_input(Some("a")), None);
    }

    #[test]
    fn plain_char_none_text_when_ime_composing() {
        let mut h = ImeHandler::new();
        h.handle_ime_preedit("ㅎ");
        assert_eq!(h.handle_plain_char_input(None), None);
    }

    #[test]
    fn plain_char_sent_when_not_composing() {
        let mut h = ImeHandler::new();
        assert_eq!(
            h.handle_plain_char_input(Some("a")),
            Some(Action::WritePty(b"a".to_vec()))
        );
    }

    #[test]
    fn plain_char_none_text_when_not_composing() {
        let mut h = ImeHandler::new();
        assert_eq!(h.handle_plain_char_input(None), None);
    }

    // === 영문 입력 흐름 (IME 비활성) ===

    #[test]
    fn english_input_flow_no_ime() {
        let mut h = ImeHandler::new();
        assert_eq!(
            h.handle_plain_char_input(Some("a")),
            Some(Action::WritePty(b"a".to_vec()))
        );
        assert_eq!(
            h.handle_plain_char_input(Some("z")),
            Some(Action::WritePty(b"z".to_vec()))
        );
        assert_eq!(
            h.handle_plain_char_input(Some("1")),
            Some(Action::WritePty(b"1".to_vec()))
        );
    }

    // === winit 버그: 첫 한글 입력 시 자소 분리 ===

    #[test]
    fn first_korean_key_leaks_jamo_before_ime_enabled() {
        let mut h = ImeHandler::new();
        let result = h.handle_plain_char_input(Some("ㅇ"));
        // 한글 자모는 pending에 저장되고 PTY에 전송되지 않음
        assert_eq!(result, None);
        assert_eq!(h.pending_jamo, Some('ㅇ'));
    }

    // === "안녕" 입력 전체 흐름: winit 실제 이벤트 순서 재현 ===

    #[test]
    fn typing_annyeong_sends_annyeong_to_pty() {
        let mut h = ImeHandler::new();
        let mut pty_bytes: Vec<u8> = Vec::new();

        // --- 첫 글자 "안" ---

        // 1) KeyboardInput("ㅇ") ← IME 활성화 전에 도착 (winit 버그)
        assert_eq!(h.handle_plain_char_input(Some("ㅇ")), None);

        // 2) Ime::Enabled

        // 3) Ime::Preedit("ㅏ") → pending "ㅇ" + "ㅏ" = "아" 로 조합
        let action = h.handle_ime_preedit("ㅏ");
        assert_eq!(action, Action::SetPreedit("아".to_string()));

        // 4) Ime::Preedit("") → Ime::Commit("ㅏ") ← 단독 자모 commit 무시
        h.handle_ime_preedit("");
        if let Action::WritePty(b) = h.handle_ime_commit("ㅏ") {
            pty_bytes.extend(b);
        }

        // 5) Ime::Preedit("ㄴ") → Ime::Commit("ㄴ") ← 단독 자모 commit 무시
        h.handle_ime_preedit("ㄴ");
        h.handle_ime_preedit("");
        if let Action::WritePty(b) = h.handle_ime_commit("ㄴ") {
            pty_bytes.extend(b);
        }

        // 6) Ime::Preedit("ㄴ") → Preedit("녀") → Preedit("녕")
        h.handle_ime_preedit("ㄴ");
        h.handle_ime_preedit("녀");
        h.handle_ime_preedit("녕");

        // 7) 조합 완료: Preedit("") → Commit("녕")
        h.handle_ime_preedit("");
        if let Action::WritePty(b) = h.handle_ime_commit("녕") {
            pty_bytes.extend(b);
        }

        // PTY에 "녕"만 전송됨 — 아직 "안"은 빠져있다
        // winit이 첫 글자 조합을 완전히 깨뜨려서 "안"은 commit되지 않음
        // preedit에서 "아"로 보정했지만 commit 경로가 없음
        //
        // TODO: preedit "아" 상태에서 다음 이벤트들이 올바르게 조합되려면
        //       winit 자체 수정이 필요하거나, 더 깊은 보정이 필요함
        assert_eq!(
            String::from_utf8(pty_bytes).unwrap(),
            "안녕"
        );
    }

    // === 한글 입력 전체 흐름 시뮬레이션 (정상 케이스) ===

    #[test]
    fn korean_input_flow_no_jamo_leak() {
        let mut h = ImeHandler::new();
        // "한" 입력 시뮬레이션 (IME 이미 활성화된 상태, 정상 동작)
        assert_eq!(h.handle_plain_char_input(Some("ㅎ")), None); // pending에 저장

        assert_eq!(h.handle_ime_preedit("ㅎ"), Action::SetPreedit("ㅎ".into()));
        assert_eq!(h.handle_ime_preedit("하"), Action::SetPreedit("하".into()));
        assert_eq!(h.handle_ime_preedit("한"), Action::SetPreedit("한".into()));

        assert_eq!(h.handle_ime_preedit(""), Action::SetPreedit(String::new()));
        assert_eq!(
            h.handle_ime_commit("한"),
            Action::WritePty("한".as_bytes().to_vec())
        );
    }
}
