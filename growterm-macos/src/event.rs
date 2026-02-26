/// macOS 윈도우에서 발생하는 이벤트
#[derive(Debug)]
pub enum AppEvent {
    /// insertText: — 조합 완료 텍스트, PTY에 전송
    TextCommit(String),
    /// setMarkedText: — 조합 중 표시
    Preedit(String),
    /// doCommandBySelector: — IME가 패스한 키, 앱이 직접 처리
    KeyInput { keycode: u16, characters: Option<String>, modifiers: Modifiers },
    /// 윈도우 리사이즈
    Resize(u32, u32),
    /// 윈도우 닫기 요청
    CloseRequested,
    /// 리드로우 요청
    RedrawRequested,
    /// 마우스 버튼 누름 (x, y in backing pixels)
    MouseDown(f64, f64),
    /// 마우스 드래그 (x, y in backing pixels)
    MouseDragged(f64, f64),
    /// 마우스 버튼 뗌 (x, y in backing pixels)
    MouseUp(f64, f64),
    /// 마우스 스크롤 (delta_y: 양수=위, 음수=아래)
    ScrollWheel(f64),
    /// 파일 드래그 앤 드롭 (파일 경로 목록)
    FileDropped(Vec<String>),
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct Modifiers: u8 {
        const SHIFT   = 0b0001;
        const CONTROL = 0b0010;
        const ALT     = 0b0100;
        const SUPER   = 0b1000;
    }
}
