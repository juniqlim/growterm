/// 한글 자모 버퍼링 및 조합
///
/// IME 서버가 아직 준비되지 않은 상태에서 단독 자모(ㅎ, ㅏ, ㄴ)가
/// insertText로 개별 커밋될 때, 이를 버퍼에 모아 음절로 조합한다.

// 초성 19자
const CHOSEONG: [char; 19] = [
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ',
    'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
];

// 중성 21자
const JUNGSEONG: [char; 21] = [
    'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ',
    'ㅝ', 'ㅞ', 'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
];

// 종성 27자 (없음 제외)
const JONGSEONG: [char; 27] = [
    'ㄱ', 'ㄲ', 'ㄳ', 'ㄴ', 'ㄵ', 'ㄶ', 'ㄷ', 'ㄹ', 'ㄺ', 'ㄻ', 'ㄼ', 'ㄽ', 'ㄾ', 'ㄿ',
    'ㅀ', 'ㅁ', 'ㅂ', 'ㅄ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
];

fn cho_index(c: char) -> Option<u32> {
    CHOSEONG.iter().position(|&x| x == c).map(|i| i as u32)
}

fn jung_index(c: char) -> Option<u32> {
    JUNGSEONG.iter().position(|&x| x == c).map(|i| i as u32)
}

fn jong_index(c: char) -> Option<u32> {
    JONGSEONG.iter().position(|&x| x == c).map(|i| (i + 1) as u32)
}

fn compose_syllable(cho: u32, jung: u32, jong: u32) -> char {
    char::from_u32(0xAC00 + cho * 21 * 28 + jung * 28 + jong).unwrap()
}

/// 단독 자모(호환 자모 U+3131..U+3163)인지 판별
pub fn is_single_jamo(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    if ('\u{3131}'..='\u{3163}').contains(&c) {
        Some(c)
    } else {
        None
    }
}

#[derive(Debug)]
enum BufState {
    Empty,
    Cho(u32),
    ChoJung(u32, u32),
    ChoJungJong(u32, u32, u32),
}

pub struct JamoBuffer {
    state: BufState,
    active: bool,
}

impl JamoBuffer {
    pub fn new() -> Self {
        Self {
            state: BufState::Empty,
            active: false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// 자모를 버퍼에 추가. 음절 완성 시 반환.
    pub fn push(&mut self, text: &str) -> Option<String> {
        self.active = true;
        let jamo = match is_single_jamo(text) {
            Some(c) => c,
            None => {
                // 단독 자모가 아님 → 플러시 후 그대로 반환
                let mut result = String::new();
                if let Some(flushed) = self.flush_inner() {
                    result.push_str(&flushed);
                }
                result.push_str(text);
                self.active = false;
                return Some(result);
            }
        };

        // 초성인지 중성인지 판별
        let is_cho = cho_index(jamo).is_some();
        let is_jung = jung_index(jamo).is_some();
        let is_jong = jong_index(jamo).is_some();

        match self.state {
            BufState::Empty => {
                if is_cho {
                    self.state = BufState::Cho(cho_index(jamo).unwrap());
                    None
                } else if is_jung {
                    // 단독 중성 → 그냥 출력
                    Some(jamo.to_string())
                } else {
                    Some(jamo.to_string())
                }
            }
            BufState::Cho(cho) => {
                if is_jung {
                    self.state = BufState::ChoJung(cho, jung_index(jamo).unwrap());
                    None
                } else if is_cho {
                    // 새 초성 → 이전 초성 플러시
                    let prev = CHOSEONG[cho as usize].to_string();
                    self.state = BufState::Cho(cho_index(jamo).unwrap());
                    Some(prev)
                } else {
                    let prev = CHOSEONG[cho as usize].to_string();
                    self.state = BufState::Empty;
                    Some(format!("{prev}{jamo}"))
                }
            }
            BufState::ChoJung(cho, jung) => {
                if is_jong {
                    self.state = BufState::ChoJungJong(cho, jung, jong_index(jamo).unwrap());
                    None
                } else if is_cho {
                    // 초중 완성 → 출력, 새 초성 시작
                    let syllable = compose_syllable(cho, jung, 0);
                    self.state = BufState::Cho(cho_index(jamo).unwrap());
                    Some(syllable.to_string())
                } else if is_jung {
                    // 중성 연속 → 초중 완성 출력, 단독 중성 출력
                    let syllable = compose_syllable(cho, jung, 0);
                    self.state = BufState::Empty;
                    Some(format!("{syllable}{jamo}"))
                } else {
                    let syllable = compose_syllable(cho, jung, 0);
                    self.state = BufState::Empty;
                    Some(format!("{syllable}{jamo}"))
                }
            }
            BufState::ChoJungJong(cho, jung, jong) => {
                if is_jung {
                    // 종성이 다음 음절의 초성이 됨
                    let syllable = compose_syllable(cho, jung, 0);
                    let new_cho = jong_to_cho(jong);
                    match new_cho {
                        Some(new_cho) => {
                            self.state =
                                BufState::ChoJung(new_cho, jung_index(jamo).unwrap());
                            Some(syllable.to_string())
                        }
                        None => {
                            self.state = BufState::Empty;
                            let jong_char = JONGSEONG[(jong - 1) as usize];
                            Some(format!("{syllable}{jong_char}{jamo}"))
                        }
                    }
                } else if is_cho {
                    // 완성 음절 출력, 새 초성
                    let syllable = compose_syllable(cho, jung, jong);
                    self.state = BufState::Cho(cho_index(jamo).unwrap());
                    Some(syllable.to_string())
                } else {
                    let syllable = compose_syllable(cho, jung, jong);
                    self.state = BufState::Empty;
                    Some(format!("{syllable}{jamo}"))
                }
            }
        }
    }

    /// 미완성 자모 강제 출력
    pub fn flush(&mut self) -> Option<String> {
        self.active = false;
        self.flush_inner()
    }

    fn flush_inner(&mut self) -> Option<String> {
        let result = match self.state {
            BufState::Empty => None,
            BufState::Cho(cho) => Some(CHOSEONG[cho as usize].to_string()),
            BufState::ChoJung(cho, jung) => Some(compose_syllable(cho, jung, 0).to_string()),
            BufState::ChoJungJong(cho, jung, jong) => {
                Some(compose_syllable(cho, jung, jong).to_string())
            }
        };
        self.state = BufState::Empty;
        result
    }
}

/// 종성 인덱스(1-based) → 초성 인덱스 변환
fn jong_to_cho(jong: u32) -> Option<u32> {
    let jong_char = JONGSEONG[(jong - 1) as usize];
    cho_index(jong_char)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_single_jamo_detects_consonants() {
        assert_eq!(is_single_jamo("ㅎ"), Some('ㅎ'));
        assert_eq!(is_single_jamo("ㄱ"), Some('ㄱ'));
    }

    #[test]
    fn is_single_jamo_detects_vowels() {
        assert_eq!(is_single_jamo("ㅏ"), Some('ㅏ'));
        assert_eq!(is_single_jamo("ㅣ"), Some('ㅣ'));
    }

    #[test]
    fn is_single_jamo_rejects_non_jamo() {
        assert_eq!(is_single_jamo("a"), None);
        assert_eq!(is_single_jamo("한"), None);
        assert_eq!(is_single_jamo("ㅎㅏ"), None);
    }

    #[test]
    fn compose_cho_jung() {
        let mut buf = JamoBuffer::new();
        assert_eq!(buf.push("ㅎ"), None);
        assert_eq!(buf.push("ㅏ"), None);
        // ㄴ은 종성 후보 → 아직 출력 안 됨
        assert_eq!(buf.push("ㄴ"), None);
        // 다음 초성이 오면 "한" 완성 출력
        assert_eq!(buf.push("ㄱ"), Some("한".into()));
        assert_eq!(buf.flush(), Some("ㄱ".into()));
    }

    #[test]
    fn compose_cho_jung_jong() {
        let mut buf = JamoBuffer::new();
        assert_eq!(buf.push("ㅎ"), None);
        assert_eq!(buf.push("ㅏ"), None);
        assert_eq!(buf.push("ㄴ"), None); // 종성 후보
        // 플러시하면 "한"
        assert_eq!(buf.flush(), Some("한".into()));
    }

    #[test]
    fn compose_hangul_full_word() {
        // "한글" = ㅎㅏㄴㄱㅡㄹ
        let mut buf = JamoBuffer::new();
        let mut result = String::new();

        for jamo in ["ㅎ", "ㅏ", "ㄴ", "ㄱ", "ㅡ", "ㄹ"] {
            if let Some(s) = buf.push(jamo) {
                result.push_str(&s);
            }
        }
        if let Some(s) = buf.flush() {
            result.push_str(&s);
        }
        assert_eq!(result, "한글");
    }

    #[test]
    fn composed_syllable_flushes_buffer() {
        let mut buf = JamoBuffer::new();
        // 버퍼에 자모가 있는 상태에서 조합된 음절이 오면 플러시
        assert_eq!(buf.push("ㅎ"), None);
        let result = buf.push("한");
        // "ㅎ" 플러시 + "한" 통과
        assert_eq!(result, Some("ㅎ한".into()));
        assert!(!buf.is_active()); // 조합된 음절이 왔으므로 비활성화
    }

    #[test]
    fn english_passes_through() {
        let mut buf = JamoBuffer::new();
        let result = buf.push("a");
        assert_eq!(result, Some("a".into()));
        assert!(!buf.is_active());
    }

    #[test]
    fn english_with_buffered_jamo_flushes() {
        let mut buf = JamoBuffer::new();
        assert_eq!(buf.push("ㅎ"), None);
        let result = buf.push("a");
        assert_eq!(result, Some("ㅎa".into()));
    }

    #[test]
    fn flush_empty_returns_none() {
        let mut buf = JamoBuffer::new();
        assert_eq!(buf.flush(), None);
    }

    #[test]
    fn flush_cho_returns_consonant() {
        let mut buf = JamoBuffer::new();
        buf.push("ㅎ");
        assert_eq!(buf.flush(), Some("ㅎ".into()));
    }

    #[test]
    fn flush_cho_jung_returns_syllable() {
        let mut buf = JamoBuffer::new();
        buf.push("ㅎ");
        buf.push("ㅏ");
        assert_eq!(buf.flush(), Some("하".into()));
    }

    #[test]
    fn jong_becomes_next_cho_when_jung_follows() {
        // ㅎㅏㄴㅏ → "하나" (ㄴ이 종성이 아닌 다음 초성)
        let mut buf = JamoBuffer::new();
        let mut result = String::new();
        for jamo in ["ㅎ", "ㅏ", "ㄴ", "ㅏ"] {
            if let Some(s) = buf.push(jamo) {
                result.push_str(&s);
            }
        }
        if let Some(s) = buf.flush() {
            result.push_str(&s);
        }
        assert_eq!(result, "하나");
    }
}
