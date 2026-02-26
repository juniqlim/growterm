use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use growterm_grid::Grid;
use growterm_macos::MacWindow;
use growterm_pty::PtyWriter;
use growterm_vt_parser::VtParser;

pub struct Tab {
    pub terminal: Arc<Mutex<TerminalState>>,
    pub pty_writer: PtyWriter,
    pub dirty: Arc<AtomicBool>,
}

pub struct TerminalState {
    pub grid: Grid,
    pub vt_parser: VtParser,
}

pub struct TabManager {
    tabs: Vec<Tab>,
    active: usize,
}

/// Info passed to the renderer for drawing the tab bar.
pub struct TabBarInfo {
    pub titles: Vec<String>,
    pub active_index: usize,
}

impl TabManager {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active: 0,
        }
    }

    pub fn add_tab(&mut self, tab: Tab) {
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
    }

    pub fn close_tab(&mut self, index: usize) -> Option<Tab> {
        if index >= self.tabs.len() {
            return None;
        }
        let tab = self.tabs.remove(index);
        if self.tabs.is_empty() {
            // caller should handle exit
        } else if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        } else if self.active > index {
            self.active -= 1;
        }
        Some(tab)
    }

    pub fn close_active(&mut self) -> Option<Tab> {
        let idx = self.active;
        self.close_tab(idx)
    }

    pub fn switch_to(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active = index;
        }
    }

    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active = (self.active + 1) % self.tabs.len();
        }
    }

    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active = if self.active == 0 {
                self.tabs.len() - 1
            } else {
                self.active - 1
            };
        }
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active)
    }

    #[allow(dead_code)]
    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    #[allow(dead_code)]
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn tabs_mut(&mut self) -> &mut [Tab] {
        &mut self.tabs
    }

    pub fn show_tab_bar(&self) -> bool {
        self.tabs.len() > 1
    }

    /// Terminal rows adjusted for tab bar presence.
    pub fn term_rows(&self, total_rows: u16) -> u16 {
        if self.show_tab_bar() {
            total_rows.saturating_sub(1).max(1)
        } else {
            total_rows
        }
    }

    /// Y pixel offset for mouse events (tab bar height or 0).
    pub fn mouse_y_offset(&self, cell_h: f32) -> f32 {
        if self.show_tab_bar() { cell_h } else { 0.0 }
    }

    pub fn tab_bar_info(&self) -> TabBarInfo {
        TabBarInfo {
            titles: (1..=self.tabs.len())
                .map(|i| if i <= 9 { format!("⌘{}", i) } else { format!("{}", i) })
                .collect(),
            active_index: self.active,
        }
    }
}

impl Tab {
    pub fn spawn(
        rows: u16,
        cols: u16,
        window: Arc<MacWindow>,
    ) -> Result<Self, std::io::Error> {
        let grid = Grid::new(cols, rows);
        let vt_parser = VtParser::new();
        let terminal = Arc::new(Mutex::new(TerminalState { grid, vt_parser }));
        let dirty = Arc::new(AtomicBool::new(false));

        let pty_writer = match growterm_pty::spawn(rows, cols) {
            Ok((reader, writer)) => {
                let responder = writer.responder();
                start_io_thread(
                    reader,
                    responder,
                    Arc::clone(&terminal),
                    Arc::clone(&dirty),
                    window,
                );
                writer
            }
            Err(e) => return Err(e),
        };

        Ok(Tab {
            terminal,
            pty_writer,
            dirty,
        })
    }
}

fn start_io_thread(
    mut reader: growterm_pty::PtyReader,
    responder: growterm_pty::PtyResponder,
    terminal: Arc<Mutex<TerminalState>>,
    dirty: Arc<AtomicBool>,
    window: Arc<MacWindow>,
) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let mut pending_queries: Vec<u8> = Vec::new();
        let mut kitty_keyboard_flags: u16 = 0;
        let mut kitty_keyboard_stack: Vec<u16> = Vec::new();
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    pending_queries.extend_from_slice(&buf[..n]);
                    let controls = extract_terminal_controls(&mut pending_queries);

                    let mut state = terminal.lock().unwrap();
                    let commands = state.vt_parser.parse(&buf[..n]);
                    for cmd in &commands {
                        state.grid.apply(cmd);
                    }
                    if state.grid.scroll_offset() == 0 {
                        state.grid.reset_scroll();
                    }
                    let cursor = state.grid.cursor_pos();
                    drop(state);

                    for control in controls {
                        match control {
                            TerminalControl::Query(query) => {
                                let response = encode_terminal_query_response(
                                    query,
                                    cursor,
                                    kitty_keyboard_flags,
                                );
                                let _ = responder.write_all_flush(response.as_bytes());
                            }
                            TerminalControl::KittyKeyboardPush(flags) => {
                                kitty_keyboard_stack.push(kitty_keyboard_flags);
                                kitty_keyboard_flags = flags;
                            }
                            TerminalControl::KittyKeyboardPop(count) => {
                                let mut remaining = count.max(1);
                                while remaining > 0 {
                                    if let Some(prev) = kitty_keyboard_stack.pop() {
                                        kitty_keyboard_flags = prev;
                                    } else {
                                        kitty_keyboard_flags = 0;
                                        break;
                                    }
                                    remaining -= 1;
                                }
                            }
                        }
                    }

                    dirty.store(true, Ordering::Relaxed);
                    window.request_redraw();
                }
                Err(e) => {
                    if e.raw_os_error() == Some(libc::EIO) {
                        break;
                    }
                    break;
                }
            }
        }
        window.request_redraw();
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalQuery {
    CursorPositionReport,
    PrimaryDeviceAttributes,
    SecondaryDeviceAttributes,
    KittyKeyboardQuery,
    ForegroundColorQuery,
    BackgroundColorQuery,
    RequestStatusStringSgr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalControl {
    Query(TerminalQuery),
    KittyKeyboardPush(u16),
    KittyKeyboardPop(u16),
}

fn extract_terminal_controls(pending: &mut Vec<u8>) -> Vec<TerminalControl> {
    let mut controls = Vec::new();
    let mut i = 0usize;
    let mut keep_from = None;

    while i < pending.len() {
        if pending[i] != 0x1b {
            i += 1;
            continue;
        }

        let rest = &pending[i..];
        if rest.starts_with(b"\x1b[6n") {
            controls.push(TerminalControl::Query(TerminalQuery::CursorPositionReport));
            i += 4;
            continue;
        }
        if rest.starts_with(b"\x1b[?u") {
            controls.push(TerminalControl::Query(TerminalQuery::KittyKeyboardQuery));
            i += 4;
            continue;
        }
        if rest.starts_with(b"\x1b[c") {
            controls.push(TerminalControl::Query(TerminalQuery::PrimaryDeviceAttributes));
            i += 3;
            continue;
        }
        if rest.starts_with(b"\x1b[>c") {
            controls.push(TerminalControl::Query(TerminalQuery::SecondaryDeviceAttributes));
            i += 4;
            continue;
        }
        if rest.starts_with(b"\x1b[>0c") {
            controls.push(TerminalControl::Query(TerminalQuery::SecondaryDeviceAttributes));
            i += 5;
            continue;
        }
        if rest.starts_with(b"\x1b]10;?\x1b\\") {
            controls.push(TerminalControl::Query(TerminalQuery::ForegroundColorQuery));
            i += 8;
            continue;
        }
        if rest.starts_with(b"\x1b]10;?\x07") {
            controls.push(TerminalControl::Query(TerminalQuery::ForegroundColorQuery));
            i += 7;
            continue;
        }
        if rest.starts_with(b"\x1b]11;?\x1b\\") {
            controls.push(TerminalControl::Query(TerminalQuery::BackgroundColorQuery));
            i += 8;
            continue;
        }
        if rest.starts_with(b"\x1b]11;?\x07") {
            controls.push(TerminalControl::Query(TerminalQuery::BackgroundColorQuery));
            i += 7;
            continue;
        }
        if rest.starts_with(b"\x1bP$qm\x1b\\") {
            controls.push(TerminalControl::Query(TerminalQuery::RequestStatusStringSgr));
            i += 7;
            continue;
        }

        match parse_kitty_keyboard_control(rest) {
            SequenceParse::Matched(control, consumed) => {
                controls.push(control);
                i += consumed;
                continue;
            }
            SequenceParse::NeedMore => {
                keep_from = Some(i);
                break;
            }
            SequenceParse::NoMatch => {}
        }

        if is_known_control_prefix(rest) {
            keep_from = Some(i);
            break;
        }

        i += 1;
    }

    if let Some(start) = keep_from {
        pending.drain(..start);
    } else {
        pending.clear();
    }

    controls
}

fn is_known_control_prefix(rest: &[u8]) -> bool {
    [
        b"\x1b[6n".as_slice(),
        b"\x1b[?u".as_slice(),
        b"\x1b[c".as_slice(),
        b"\x1b[>c".as_slice(),
        b"\x1b[>0c".as_slice(),
        b"\x1b]10;?\x1b\\".as_slice(),
        b"\x1b]10;?\x07".as_slice(),
        b"\x1b]11;?\x1b\\".as_slice(),
        b"\x1b]11;?\x07".as_slice(),
        b"\x1bP$qm\x1b\\".as_slice(),
    ]
        .iter()
        .any(|pat| pat.starts_with(rest))
        || is_kitty_keyboard_control_prefix(rest)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SequenceParse<T> {
    Matched(T, usize),
    NeedMore,
    NoMatch,
}

fn parse_kitty_keyboard_control(rest: &[u8]) -> SequenceParse<TerminalControl> {
    if !rest.starts_with(b"\x1b[") {
        return SequenceParse::NoMatch;
    }
    if rest.len() < 3 {
        return SequenceParse::NeedMore;
    }
    let mode = rest[2];
    if mode != b'>' && mode != b'<' {
        return SequenceParse::NoMatch;
    }
    if rest.len() == 3 {
        return SequenceParse::NeedMore;
    }

    let mut idx = 3usize;
    while idx < rest.len() && rest[idx].is_ascii_digit() {
        idx += 1;
    }
    if idx == rest.len() {
        return SequenceParse::NeedMore;
    }
    if rest[idx] != b'u' {
        return SequenceParse::NoMatch;
    }

    let digits = &rest[3..idx];
    if mode == b'>' && digits.is_empty() {
        return SequenceParse::NoMatch;
    }

    let value = if digits.is_empty() {
        1
    } else {
        parse_u16_saturating(digits)
    };

    let control = if mode == b'>' {
        TerminalControl::KittyKeyboardPush(value)
    } else {
        TerminalControl::KittyKeyboardPop(value)
    };
    SequenceParse::Matched(control, idx + 1)
}

fn is_kitty_keyboard_control_prefix(rest: &[u8]) -> bool {
    if !b"\x1b[".starts_with(rest) {
        return false;
    }
    if rest.len() <= 2 {
        return true;
    }
    let mode = rest[2];
    if mode != b'>' && mode != b'<' {
        return false;
    }
    rest[3..]
        .iter()
        .all(|byte| byte.is_ascii_digit() || *byte == b'u')
}

fn parse_u16_saturating(bytes: &[u8]) -> u16 {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .map(|n| n.min(u16::MAX as u32) as u16)
        .unwrap_or(0)
}

fn encode_terminal_query_response(
    query: TerminalQuery,
    cursor: (u16, u16),
    kitty_keyboard_flags: u16,
) -> String {
    match query {
        TerminalQuery::CursorPositionReport => {
            let row = cursor.0.saturating_add(1);
            let col = cursor.1.saturating_add(1);
            format!("\x1b[{row};{col}R")
        }
        TerminalQuery::PrimaryDeviceAttributes => "\x1b[?1;2c".to_string(),
        TerminalQuery::SecondaryDeviceAttributes => "\x1b[>0;95;0c".to_string(),
        TerminalQuery::KittyKeyboardQuery => format!("\x1b[?{kitty_keyboard_flags}u"),
        TerminalQuery::ForegroundColorQuery => "\x1b]10;rgb:cccc/cccc/cccc\x07".to_string(),
        TerminalQuery::BackgroundColorQuery => "\x1b]11;rgb:0000/0000/0000\x07".to_string(),
        TerminalQuery::RequestStatusStringSgr => "\x1bP1$r0m\x1b\\".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_manager_is_empty() {
        let mgr = TabManager::new();
        assert!(mgr.is_empty());
        assert_eq!(mgr.tab_count(), 0);
        assert!(mgr.active_tab().is_none());
    }

    fn dummy_tab() -> Tab {
        let grid = Grid::new(80, 24);
        let vt_parser = VtParser::new();
        let terminal = Arc::new(Mutex::new(TerminalState { grid, vt_parser }));
        let dirty = Arc::new(AtomicBool::new(false));
        // We can't create a real PtyWriter without spawning, so we test TabManager logic
        // separately. For unit tests we'll test TabManager methods that don't need PtyWriter.
        // Instead, create a stub by spawning a real PTY (acceptable for unit test).
        let (_, pty_writer) = growterm_pty::spawn(24, 80).unwrap();
        Tab {
            terminal,
            pty_writer,
            dirty,
        }
    }

    #[test]
    fn add_tab_activates_new_tab() {
        let mut mgr = TabManager::new();
        mgr.add_tab(dummy_tab());
        assert_eq!(mgr.tab_count(), 1);
        assert_eq!(mgr.active_index(), 0);

        mgr.add_tab(dummy_tab());
        assert_eq!(mgr.tab_count(), 2);
        assert_eq!(mgr.active_index(), 1);
    }

    #[test]
    fn switch_to_valid_index() {
        let mut mgr = TabManager::new();
        mgr.add_tab(dummy_tab());
        mgr.add_tab(dummy_tab());
        mgr.add_tab(dummy_tab());

        mgr.switch_to(0);
        assert_eq!(mgr.active_index(), 0);

        mgr.switch_to(2);
        assert_eq!(mgr.active_index(), 2);
    }

    #[test]
    fn switch_to_invalid_index_no_change() {
        let mut mgr = TabManager::new();
        mgr.add_tab(dummy_tab());
        mgr.switch_to(5);
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn next_prev_tab_wraps() {
        let mut mgr = TabManager::new();
        mgr.add_tab(dummy_tab());
        mgr.add_tab(dummy_tab());
        mgr.add_tab(dummy_tab());

        mgr.switch_to(0);

        mgr.next_tab();
        assert_eq!(mgr.active_index(), 1);
        mgr.next_tab();
        assert_eq!(mgr.active_index(), 2);
        mgr.next_tab();
        assert_eq!(mgr.active_index(), 0); // wrap

        mgr.prev_tab();
        assert_eq!(mgr.active_index(), 2); // wrap back
        mgr.prev_tab();
        assert_eq!(mgr.active_index(), 1);
    }

    #[test]
    fn close_tab_adjusts_active() {
        let mut mgr = TabManager::new();
        mgr.add_tab(dummy_tab());
        mgr.add_tab(dummy_tab());
        mgr.add_tab(dummy_tab());

        // Active is 2 (last added). Close tab 1.
        let removed = mgr.close_tab(1);
        assert!(removed.is_some());
        assert_eq!(mgr.tab_count(), 2);
        // active was 2, index 1 was removed, so active adjusts to 1
        assert_eq!(mgr.active_index(), 1);
    }

    #[test]
    fn close_active_tab() {
        let mut mgr = TabManager::new();
        mgr.add_tab(dummy_tab());
        mgr.add_tab(dummy_tab());
        mgr.switch_to(0);

        let removed = mgr.close_active();
        assert!(removed.is_some());
        assert_eq!(mgr.tab_count(), 1);
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn close_last_remaining_tab() {
        let mut mgr = TabManager::new();
        mgr.add_tab(dummy_tab());
        let removed = mgr.close_active();
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn tab_bar_info_reflects_state() {
        let mut mgr = TabManager::new();
        mgr.add_tab(dummy_tab());
        mgr.add_tab(dummy_tab());

        let info = mgr.tab_bar_info();
        assert_eq!(info.titles, vec!["⌘1", "⌘2"]);
        assert_eq!(info.active_index, 1);
    }

    #[test]
    fn extract_terminal_queries_detects_known_queries() {
        let mut pending = b"\x1b[6n\x1b[?u\x1b[c\x1b[>0c".to_vec();
        let controls = extract_terminal_controls(&mut pending);
        assert_eq!(
            controls,
            vec![
                TerminalControl::Query(TerminalQuery::CursorPositionReport),
                TerminalControl::Query(TerminalQuery::KittyKeyboardQuery),
                TerminalControl::Query(TerminalQuery::PrimaryDeviceAttributes),
                TerminalControl::Query(TerminalQuery::SecondaryDeviceAttributes),
            ]
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn extract_terminal_queries_keeps_partial_sequence() {
        let mut pending = b"\x1b[6".to_vec();
        let controls = extract_terminal_controls(&mut pending);
        assert!(controls.is_empty());
        assert_eq!(pending, b"\x1b[6");
    }

    #[test]
    fn extract_terminal_queries_detects_osc_color_queries_with_bel() {
        let mut pending = b"\x1b]10;?\x07\x1b]11;?\x07".to_vec();
        let controls = extract_terminal_controls(&mut pending);
        assert_eq!(
            controls,
            vec![
                TerminalControl::Query(TerminalQuery::ForegroundColorQuery),
                TerminalControl::Query(TerminalQuery::BackgroundColorQuery)
            ]
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn extract_terminal_controls_detects_kitty_push_query_pop() {
        let mut pending = b"\x1b[>7u\x1b[?u\x1b[<1u".to_vec();
        let controls = extract_terminal_controls(&mut pending);
        assert_eq!(
            controls,
            vec![
                TerminalControl::KittyKeyboardPush(7),
                TerminalControl::Query(TerminalQuery::KittyKeyboardQuery),
                TerminalControl::KittyKeyboardPop(1)
            ]
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn extract_terminal_controls_detects_decrqss_sgr_query() {
        let mut pending = b"\x1bP$qm\x1b\\".to_vec();
        let controls = extract_terminal_controls(&mut pending);
        assert_eq!(
            controls,
            vec![TerminalControl::Query(TerminalQuery::RequestStatusStringSgr)]
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn extract_terminal_controls_keeps_partial_kitty_push() {
        let mut pending = b"\x1b[>7".to_vec();
        let controls = extract_terminal_controls(&mut pending);
        assert!(controls.is_empty());
        assert_eq!(pending, b"\x1b[>7");
    }

    #[test]
    fn kitty_keyboard_query_response_uses_runtime_flags() {
        let response = encode_terminal_query_response(TerminalQuery::KittyKeyboardQuery, (0, 0), 7);
        assert_eq!(response, "\x1b[?7u");
    }

    #[test]
    fn osc_query_responses_use_bel_terminator() {
        let fg = encode_terminal_query_response(TerminalQuery::ForegroundColorQuery, (0, 0), 0);
        let bg = encode_terminal_query_response(TerminalQuery::BackgroundColorQuery, (0, 0), 0);
        assert_eq!(fg, "\x1b]10;rgb:cccc/cccc/cccc\x07");
        assert_eq!(bg, "\x1b]11;rgb:0000/0000/0000\x07");
    }

    #[test]
    fn decrqss_sgr_response_is_supported() {
        let response =
            encode_terminal_query_response(TerminalQuery::RequestStatusStringSgr, (0, 0), 0);
        assert_eq!(response, "\x1bP1$r0m\x1b\\");
    }
}
