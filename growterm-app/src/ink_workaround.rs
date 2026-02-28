// Workaround for Claude Code's React Ink placing the terminal cursor
// at the wrong position during IME composition.
// Remove this entire module once Claude Code fixes cursor positioning.

use growterm_types::Cell;
use unicode_width::UnicodeWidthChar;

const CLAUDE_PROCESS_NAME: &str = "claude";
const CLAUDE_PROMPT_BASE_COL: u16 = 2;

pub struct InkImeState {
    ink_app_cached: Option<bool>,
    committed_width: u16,
}

impl InkImeState {
    pub fn new() -> Self {
        Self {
            ink_app_cached: None,
            committed_width: 0,
        }
    }

    /// Track committed text width for preedit offset calculation.
    pub fn on_text_commit(&mut self, text: &str) {
        if self.ink_app_cached == Some(true) {
            let w: u16 = text.chars().map(|c| c.width().unwrap_or(1) as u16).sum();
            self.committed_width = self.committed_width.saturating_add(w);
        }
    }

    /// Detect whether the active PTY is running a Claude Code process.
    pub fn on_preedit(&mut self, child_pid: Option<u32>) {
        self.on_preedit_with(child_pid, has_descendant_named);
    }

    fn on_preedit_with(&mut self, child_pid: Option<u32>, checker: impl FnOnce(u32, &str) -> bool) {
        if let Some(pid) = child_pid {
            let found = checker(pid, CLAUDE_PROCESS_NAME);
            self.ink_app_cached = Some(found);
            if !found {
                self.committed_width = 0;
            }
        }
    }

    /// Reset committed width on Enter.
    pub fn on_enter(&mut self) {
        self.committed_width = 0;
    }

    pub fn is_active(&self) -> bool {
        self.ink_app_cached == Some(true)
    }

    /// Calculate overridden preedit position for Ink apps.
    /// Returns `Some((row, col))` if override is needed.
    pub fn preedit_pos(&self, cells: &[Vec<Cell>], cols: u16) -> Option<(u16, u16)> {
        if !self.is_active() {
            return None;
        }
        find_prompt_row(cells).map(|prompt_row| {
            let col = CLAUDE_PROMPT_BASE_COL + self.committed_width;
            let row = prompt_row as u16 + col / cols;
            let col = col % cols;
            (row, col)
        })
    }
}

/// Walk the process tree to check if any descendant has the given name.
fn has_descendant_named(root_pid: u32, name: &str) -> bool {
    let output = match std::process::Command::new("ps")
        .args(["-eo", "pid,ppid,comm="])
        .output()
    {
        Ok(o) => o,
        Err(_) => return false,
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut children: std::collections::HashMap<u32, Vec<(u32, String)>> =
        std::collections::HashMap::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            continue;
        }
        let pid: u32 = match parts[0].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let ppid: u32 = match parts[1].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let comm = parts[2..].join(" ");
        children.entry(ppid).or_default().push((pid, comm));
    }
    let mut stack = vec![root_pid];
    while let Some(pid) = stack.pop() {
        if let Some(kids) = children.get(&pid) {
            for (kid_pid, comm) in kids {
                if comm.contains(name) {
                    return true;
                }
                stack.push(*kid_pid);
            }
        }
    }
    false
}

/// Find the prompt row (❯) between two separator lines (─) in the grid.
fn find_prompt_row(cells: &[Vec<Cell>]) -> Option<usize> {
    let is_separator = |row: &[Cell]| -> bool {
        row.first().map_or(false, |c| c.character == '─')
    };
    let separators: Vec<usize> = cells
        .iter()
        .enumerate()
        .filter(|(_, row)| is_separator(row))
        .map(|(i, _)| i)
        .collect();
    for window in separators.windows(2).rev() {
        let (top, bottom) = (window[0], window[1]);
        for row_idx in (top + 1)..bottom {
            if cells[row_idx].iter().any(|c| c.character == '❯') {
                return Some(row_idx);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use growterm_types::Cell;

    fn make_row(chars: &str, width: usize) -> Vec<Cell> {
        let mut row = vec![Cell::default(); width];
        for (i, ch) in chars.chars().enumerate() {
            if i < width {
                row[i].character = ch;
            }
        }
        row
    }

    #[test]
    fn find_prompt_row_between_separators() {
        let cells = vec![
            make_row("hello", 80),
            make_row("─────", 80),
            make_row("❯ ", 80),
            make_row("─────", 80),
            make_row("output", 80),
        ];
        assert_eq!(find_prompt_row(&cells), Some(2));
    }

    #[test]
    fn find_prompt_row_no_separators() {
        let cells = vec![
            make_row("hello", 80),
            make_row("❯ ", 80),
        ];
        assert_eq!(find_prompt_row(&cells), None);
    }

    #[test]
    fn find_prompt_row_no_prompt() {
        let cells = vec![
            make_row("─────", 80),
            make_row("hello", 80),
            make_row("─────", 80),
        ];
        assert_eq!(find_prompt_row(&cells), None);
    }

    #[test]
    fn preedit_pos_not_active() {
        let state = InkImeState::new();
        let cells = vec![make_row("─────", 80), make_row("❯ ", 80), make_row("─────", 80)];
        assert_eq!(state.preedit_pos(&cells, 80), None);
    }

    #[test]
    fn preedit_pos_with_committed_width() {
        let state = InkImeState {
            ink_app_cached: Some(true),
            committed_width: 4,
        };
        let cells = vec![
            make_row("─────", 80),
            make_row("❯ hello", 80),
            make_row("─────", 80),
        ];
        // col = 2 + 4 = 6, row = 1 + 6/80 = 1, col = 6 % 80 = 6
        assert_eq!(state.preedit_pos(&cells, 80), Some((1, 6)));
    }

    #[test]
    fn on_text_commit_accumulates_width() {
        let mut state = InkImeState {
            ink_app_cached: Some(true),
            committed_width: 0,
        };
        state.on_text_commit("한"); // width 2
        assert_eq!(state.committed_width, 2);
        state.on_text_commit("ab"); // width 2
        assert_eq!(state.committed_width, 4);
    }

    #[test]
    fn on_preedit_detects_claude() {
        let mut state = InkImeState::new();
        state.on_preedit_with(Some(1), |_, _| true);
        assert!(state.is_active());
    }

    #[test]
    fn on_preedit_resets_width_when_not_claude() {
        let mut state = InkImeState {
            ink_app_cached: Some(true),
            committed_width: 5,
        };
        state.on_preedit_with(Some(1), |_, _| false);
        assert!(!state.is_active());
        assert_eq!(state.committed_width, 0);
    }

    #[test]
    fn on_preedit_noop_without_pid() {
        let mut state = InkImeState::new();
        state.on_preedit_with(None, |_, _| panic!("should not be called"));
        assert_eq!(state.ink_app_cached, None);
    }

    #[test]
    fn on_enter_resets_width() {
        let mut state = InkImeState {
            ink_app_cached: Some(true),
            committed_width: 10,
        };
        state.on_enter();
        assert_eq!(state.committed_width, 0);
    }
}
