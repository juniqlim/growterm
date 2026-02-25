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
    pub title: String,
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

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn tabs_mut(&mut self) -> &mut [Tab] {
        &mut self.tabs
    }

    pub fn tab_bar_info(&self) -> TabBarInfo {
        TabBarInfo {
            titles: self.tabs.iter().map(|t| t.title.clone()).collect(),
            active_index: self.active,
        }
    }
}

impl Tab {
    pub fn spawn(
        rows: u16,
        cols: u16,
        index: usize,
        window: Arc<MacWindow>,
    ) -> Result<Self, std::io::Error> {
        let grid = Grid::new(cols, rows);
        let vt_parser = VtParser::new();
        let terminal = Arc::new(Mutex::new(TerminalState { grid, vt_parser }));
        let dirty = Arc::new(AtomicBool::new(false));

        let pty_writer = match growterm_pty::spawn(rows, cols) {
            Ok((reader, writer)) => {
                start_io_thread(reader, Arc::clone(&terminal), Arc::clone(&dirty), window);
                writer
            }
            Err(e) => return Err(e),
        };

        Ok(Tab {
            terminal,
            pty_writer,
            dirty,
            title: format!("Tab {}", index + 1),
        })
    }
}

fn start_io_thread(
    mut reader: growterm_pty::PtyReader,
    terminal: Arc<Mutex<TerminalState>>,
    dirty: Arc<AtomicBool>,
    window: Arc<MacWindow>,
) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let mut state = terminal.lock().unwrap();
                    let commands = state.vt_parser.parse(&buf[..n]);
                    for cmd in &commands {
                        state.grid.apply(cmd);
                    }
                    state.grid.reset_scroll();
                    drop(state);
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

    fn dummy_tab(title: &str) -> Tab {
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
            title: title.to_string(),
        }
    }

    #[test]
    fn add_tab_activates_new_tab() {
        let mut mgr = TabManager::new();
        mgr.add_tab(dummy_tab("Tab 1"));
        assert_eq!(mgr.tab_count(), 1);
        assert_eq!(mgr.active_index(), 0);

        mgr.add_tab(dummy_tab("Tab 2"));
        assert_eq!(mgr.tab_count(), 2);
        assert_eq!(mgr.active_index(), 1);
    }

    #[test]
    fn switch_to_valid_index() {
        let mut mgr = TabManager::new();
        mgr.add_tab(dummy_tab("Tab 1"));
        mgr.add_tab(dummy_tab("Tab 2"));
        mgr.add_tab(dummy_tab("Tab 3"));

        mgr.switch_to(0);
        assert_eq!(mgr.active_index(), 0);

        mgr.switch_to(2);
        assert_eq!(mgr.active_index(), 2);
    }

    #[test]
    fn switch_to_invalid_index_no_change() {
        let mut mgr = TabManager::new();
        mgr.add_tab(dummy_tab("Tab 1"));
        mgr.switch_to(5);
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn next_prev_tab_wraps() {
        let mut mgr = TabManager::new();
        mgr.add_tab(dummy_tab("Tab 1"));
        mgr.add_tab(dummy_tab("Tab 2"));
        mgr.add_tab(dummy_tab("Tab 3"));

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
        mgr.add_tab(dummy_tab("Tab 1"));
        mgr.add_tab(dummy_tab("Tab 2"));
        mgr.add_tab(dummy_tab("Tab 3"));

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
        mgr.add_tab(dummy_tab("Tab 1"));
        mgr.add_tab(dummy_tab("Tab 2"));
        mgr.switch_to(0);

        let removed = mgr.close_active();
        assert!(removed.is_some());
        assert_eq!(mgr.tab_count(), 1);
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn close_last_remaining_tab() {
        let mut mgr = TabManager::new();
        mgr.add_tab(dummy_tab("Tab 1"));
        let removed = mgr.close_active();
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn tab_bar_info_reflects_state() {
        let mut mgr = TabManager::new();
        mgr.add_tab(dummy_tab("Shell"));
        mgr.add_tab(dummy_tab("Vim"));

        let info = mgr.tab_bar_info();
        assert_eq!(info.titles, vec!["Shell", "Vim"]);
        assert_eq!(info.active_index, 1);
    }
}
