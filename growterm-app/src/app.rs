use std::io::Write;
use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc};

use growterm_gpu_draw::{GpuDrawer, TabBarInfo};
use growterm_macos::{AppEvent, MacWindow, Modifiers};

use unicode_width::UnicodeWidthChar;

use crate::selection::{self, Selection};
use crate::tab::{Tab, TabManager};
use crate::zoom;

pub fn run(window: Arc<MacWindow>, rx: mpsc::Receiver<AppEvent>, mut drawer: GpuDrawer) {
    let (cell_w, cell_h) = drawer.cell_size();
    let mut font_size = crate::FONT_SIZE;
    let (width, height) = window.inner_size();

    let (cols, rows) = zoom::calc_grid_size(width, height, cell_w, cell_h);

    let mut tabs = TabManager::new();

    // Spawn initial tab (no tab bar for single tab)
    match Tab::spawn(rows, cols, window.clone()) {
        Ok(tab) => {
            tabs.add_tab(tab);
        }
        Err(e) => {
            eprintln!("Failed to spawn PTY: {e}");
            return;
        }
    }

    let mut preedit = String::new();
    let mut prev_preedit = String::new();
    let mut sel = Selection::default();
    let mut scroll_accum: f64 = 0.0;
    let mut deferred: Option<AppEvent> = None;
    let grid_dump_path = std::env::var("GROWTERM_GRID_DUMP").ok();
    let test_input = std::env::var("GROWTERM_TEST_INPUT").ok();
    let test_dropped_path = std::env::var("GROWTERM_TEST_DROPPED_PATH").ok();
    let mut test_input_sent = false;
    let mut test_drop_sent = false;
    let mut ink_app_cached: Option<bool> = None;
    let mut ink_committed_width: u16 = 0;

    loop {
        let event = if let Some(evt) = deferred.take() {
            evt
        } else {
            match rx.recv() {
                Ok(evt) => evt,
                Err(_) => break,
            }
        };
        match event {
            AppEvent::TextCommit(text) => {
                preedit.clear();
                if ink_app_cached == Some(true) {
                    let w: u16 = text.chars().map(|c| c.width().unwrap_or(1) as u16).sum();
                    ink_committed_width += w;
                }
                if let Some(tab) = tabs.active_tab_mut() {
                    let _ = tab.pty_writer.write_all(text.as_bytes());
                    let _ = tab.pty_writer.flush();
                }
            }
            AppEvent::Preedit(text) => {
                if !text.is_empty() {
                    if let Some(tab) = tabs.active_tab() {
                        if let Some(pid) = tab.pty_writer.child_pid() {
                            let found = has_descendant_named(pid, "claude");
                            ink_app_cached = Some(found);
                            if !found {
                                ink_committed_width = 0;
                            }
                        }
                    }
                }
                if !text.is_empty() {
                    if let Some(tab) = tabs.active_tab() {
                        let state = tab.terminal.lock().unwrap();
                        let (crow, ccol) = state.grid.cursor_pos();
                        let cells = state.grid.cells();
                        eprintln!("=== PREEDIT cursor=({},{}) ===", crow, ccol);
                        // 커서 행 주변 5행의 셀 정보 출력
                        let start = (crow as usize).saturating_sub(5);
                        let end = (crow as usize + 3).min(cells.len());
                        for r in start..end {
                            let row = &cells[r];
                            for (c, cell) in row.iter().enumerate().take(60) {
                                if cell.character != '\0' && cell.character != ' ' {
                                    eprintln!(
                                        "  [{},{}] ch='{}' fg={:?} bg={:?}",
                                        r, c, cell.character,
                                        cell.fg, cell.bg,
                                    );
                                }
                            }
                        }
                        // 커서 위치 셀
                        if (crow as usize) < cells.len() {
                            let cursor_cell = &cells[crow as usize][ccol as usize];
                            eprintln!(
                                "  CURSOR CELL ch='{}' fg={:?} bg={:?}",
                                cursor_cell.character,
                                cursor_cell.fg, cursor_cell.bg,
                            );
                        }
                    }
                }
                preedit = text;
                window.request_redraw();
            }
            AppEvent::KeyInput {
                keycode,
                characters,
                modifiers,
            } => {
                use growterm_macos::key_convert::keycode as kc;

                if modifiers.contains(Modifiers::SUPER) {
                    // Cmd+T: new tab
                    if keycode == kc::ANSI_T {
                        let (cw, ch) = drawer.cell_size();
                        let (w, h) = window.inner_size();
                        let (cols, rows) = zoom::calc_grid_size(w, h, cw, ch);
                        let had_no_tab_bar = !tabs.show_tab_bar();
                        let term_rows = rows.saturating_sub(1).max(1);
                        match Tab::spawn(term_rows, cols, window.clone()) {
                            Ok(tab) => {
                                tabs.add_tab(tab);
                                sel.clear();
                                preedit.clear();
                                // Tab bar just appeared — shrink existing tabs by 1 row
                                if had_no_tab_bar && tabs.show_tab_bar() {
                                    for t in tabs.tabs_mut() {
                                        let mut st = t.terminal.lock().unwrap();
                                        st.grid.resize(cols, term_rows);
                                        drop(st);
                                        let _ = t.pty_writer.resize(term_rows, cols);
                                    }
                                }
                            }
                            Err(e) => eprintln!("Failed to spawn tab: {e}"),
                        }
                        render_with_tabs(&mut drawer, &tabs, &preedit, &sel, ink_app_cached.unwrap_or(false), ink_committed_width);
                        continue;
                    }

                    // Cmd+W: close tab
                    if keycode == kc::ANSI_W {
                        let had_tab_bar = tabs.show_tab_bar();
                        tabs.close_active();
                        if tabs.is_empty() {
                            std::process::exit(0);
                        }
                        sel.clear();
                        preedit.clear();
                        // Tab bar just disappeared — expand remaining tab by 1 row
                        if had_tab_bar && !tabs.show_tab_bar() {
                            let (cw, ch) = drawer.cell_size();
                            let (w, h) = window.inner_size();
                            let (cols, rows) = zoom::calc_grid_size(w, h, cw, ch);
                            if let Some(t) = tabs.active_tab_mut() {
                                let mut st = t.terminal.lock().unwrap();
                                st.grid.resize(cols, rows);
                                drop(st);
                                let _ = t.pty_writer.resize(rows, cols);
                            }
                        }
                        render_with_tabs(&mut drawer, &tabs, &preedit, &sel, ink_app_cached.unwrap_or(false), ink_committed_width);
                        continue;
                    }

                    // Cmd+Shift+[ / Cmd+Shift+]: prev/next tab
                    if modifiers.contains(Modifiers::SHIFT) {
                        if keycode == kc::ANSI_LEFT_BRACKET {
                            tabs.prev_tab();
                            sel.clear();
                            preedit.clear();
                            render_with_tabs(&mut drawer, &tabs, &preedit, &sel, ink_app_cached.unwrap_or(false), ink_committed_width);
                            continue;
                        }
                        if keycode == kc::ANSI_RIGHT_BRACKET {
                            tabs.next_tab();
                            sel.clear();
                            preedit.clear();
                            render_with_tabs(&mut drawer, &tabs, &preedit, &sel, ink_app_cached.unwrap_or(false), ink_committed_width);
                            continue;
                        }
                    }

                    // Cmd+1~9: switch to tab by number
                    let tab_num = match keycode {
                        k if k == kc::ANSI_1 => Some(0),
                        k if k == kc::ANSI_2 => Some(1),
                        k if k == kc::ANSI_3 => Some(2),
                        k if k == kc::ANSI_4 => Some(3),
                        k if k == kc::ANSI_5 => Some(4),
                        k if k == kc::ANSI_6 => Some(5),
                        k if k == kc::ANSI_7 => Some(6),
                        k if k == kc::ANSI_8 => Some(7),
                        k if k == kc::ANSI_9 => Some(8),
                        _ => None,
                    };
                    if let Some(idx) = tab_num {
                        if idx < tabs.tab_count() {
                            tabs.switch_to(idx);
                            sel.clear();
                            preedit.clear();
                            render_with_tabs(&mut drawer, &tabs, &preedit, &sel, ink_app_cached.unwrap_or(false), ink_committed_width);
                        }
                        continue;
                    }

                    // Cmd+PageUp/Down: scroll
                    if keycode == kc::PAGE_UP || keycode == kc::PAGE_DOWN {
                        if let Some(tab) = tabs.active_tab() {
                            let mut state = tab.terminal.lock().unwrap();
                            let row_count = state.grid.cells().len();
                            if keycode == kc::PAGE_UP {
                                state.grid.scroll_up_view(row_count);
                            } else {
                                state.grid.scroll_down_view(row_count);
                            }
                        }
                        render_with_tabs(&mut drawer, &tabs, &preedit, &sel, ink_app_cached.unwrap_or(false), ink_committed_width);
                        continue;
                    }

                    // Cmd+C copy
                    if keycode == kc::ANSI_C {
                        if !sel.is_empty() {
                            if let Some(tab) = tabs.active_tab() {
                                let state = tab.terminal.lock().unwrap();
                                let text = selection::extract_text_absolute(&state.grid, &sel);
                                drop(state);
                                if !text.is_empty() {
                                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                        let _ = clipboard.set_text(text);
                                    }
                                }
                            }
                        }
                        continue;
                    }

                    // Cmd+V paste
                    if keycode == kc::ANSI_V {
                        if let Ok(mut clipboard) = arboard::Clipboard::new() {
                            if let Ok(text) = clipboard.get_text() {
                                if !text.is_empty() {
                                    if let Some(tab) = tabs.active_tab_mut() {
                                        let _ = tab.pty_writer.write_all(text.as_bytes());
                                        let _ = tab.pty_writer.flush();
                                    }
                                }
                            }
                        }
                        continue;
                    }

                    // Cmd+= / Cmd+- (zoom)
                    let zoom_delta = match keycode {
                        k if k == kc::ANSI_EQUAL => Some(2.0f32),
                        k if k == kc::ANSI_MINUS => Some(-2.0f32),
                        _ => None,
                    };
                    if let Some(delta) = zoom_delta {
                        font_size = zoom::apply_zoom(font_size, delta);
                        drawer.set_font_size(font_size);
                        let (cw, ch) = drawer.cell_size();
                        let (w, h) = window.inner_size();
                        let (cols, rows) = zoom::calc_grid_size(w, h, cw, ch);
                        let term_rows = tabs.term_rows(rows);
                        // Resize all tabs
                        for tab in tabs.tabs_mut() {
                            let mut state = tab.terminal.lock().unwrap();
                            state.grid.resize(cols, term_rows);
                            drop(state);
                            let _ = tab.pty_writer.resize(term_rows, cols);
                        }
                        render_with_tabs(&mut drawer, &tabs, &preedit, &sel, ink_app_cached.unwrap_or(false), ink_committed_width);
                        continue;
                    }
                    continue;
                }

                if let Some(key_event) =
                    growterm_macos::convert_key(keycode, characters.as_deref(), modifiers)
                {
                    let bytes = growterm_input::encode(key_event);
                    // Enter 시 ink commit 폭 리셋
                    if bytes == b"\r" || bytes == b"\n" {
                        eprintln!("[ENTER] resetting ink_committed_width from {}", ink_committed_width);
                        ink_committed_width = 0;
                    } else {
                        eprintln!("[KEY] bytes={:?} ink_committed_width={}", bytes, ink_committed_width);
                    }
                    if let Some(tab) = tabs.active_tab_mut() {
                        let _ = tab.pty_writer.write_all(&bytes);
                        let _ = tab.pty_writer.flush();
                    }
                }
            }
            AppEvent::MouseDown(x, y) => {
                let (cw, ch) = drawer.cell_size();
                let (screen_row, col) =
                    selection::pixel_to_cell(x as f32, y as f32 - tabs.mouse_y_offset(ch), cw, ch);
                let abs_row = if let Some(tab) = tabs.active_tab() {
                    let state = tab.terminal.lock().unwrap();
                    let base = state
                        .grid
                        .scrollback_len()
                        .saturating_sub(state.grid.scroll_offset());
                    screen_row as u32 + base as u32
                } else {
                    screen_row as u32
                };
                sel.begin(abs_row, col);
                window.request_redraw();
            }
            AppEvent::MouseDragged(x, y) => {
                if sel.active {
                    let (cw, ch) = drawer.cell_size();
                    let (screen_row, col) = selection::pixel_to_cell(
                        x as f32,
                        y as f32 - tabs.mouse_y_offset(ch),
                        cw,
                        ch,
                    );
                    let abs_row = if let Some(tab) = tabs.active_tab() {
                        let state = tab.terminal.lock().unwrap();
                        let base = state
                            .grid
                            .scrollback_len()
                            .saturating_sub(state.grid.scroll_offset());
                        screen_row as u32 + base as u32
                    } else {
                        screen_row as u32
                    };
                    sel.update(abs_row, col);
                    window.request_redraw();
                }
            }
            AppEvent::MouseUp(x, y) => {
                let (cw, ch) = drawer.cell_size();
                let (screen_row, col) =
                    selection::pixel_to_cell(x as f32, y as f32 - tabs.mouse_y_offset(ch), cw, ch);
                let abs_row = if let Some(tab) = tabs.active_tab() {
                    let state = tab.terminal.lock().unwrap();
                    let base = state
                        .grid
                        .scrollback_len()
                        .saturating_sub(state.grid.scroll_offset());
                    screen_row as u32 + base as u32
                } else {
                    screen_row as u32
                };
                sel.update(abs_row, col);
                sel.finish();
                window.request_redraw();
            }
            AppEvent::ScrollWheel(delta_y) => {
                scroll_accum += delta_y;
                let (_, ch) = drawer.cell_size();
                let line_height = if ch > 0.0 { ch as f64 } else { 20.0 };
                let lines = (scroll_accum / line_height).trunc() as i32;
                if lines != 0 {
                    scroll_accum -= lines as f64 * line_height;
                    if let Some(tab) = tabs.active_tab() {
                        let mut state = tab.terminal.lock().unwrap();
                        if lines > 0 {
                            state.grid.scroll_up_view(lines as usize);
                        } else {
                            state.grid.scroll_down_view((-lines) as usize);
                        }
                    }
                    render_with_tabs(&mut drawer, &tabs, &preedit, &sel, ink_app_cached.unwrap_or(false), ink_committed_width);
                }
            }
            AppEvent::Resize(mut w, mut h) => {
                loop {
                    match rx.try_recv() {
                        Ok(AppEvent::Resize(nw, nh)) => {
                            w = nw;
                            h = nh;
                        }
                        Ok(other) => {
                            deferred = Some(other);
                            break;
                        }
                        Err(_) => break,
                    }
                }
                drawer.resize(w, h);
                let (cw, ch) = drawer.cell_size();
                let (cols, rows) = zoom::calc_grid_size(w, h, cw, ch);
                let term_rows = tabs.term_rows(rows);
                for tab in tabs.tabs_mut() {
                    let mut state = tab.terminal.lock().unwrap();
                    state.grid.resize(cols, term_rows);
                    drop(state);
                    let _ = tab.pty_writer.resize(term_rows, cols);
                }
                render_with_tabs(&mut drawer, &tabs, &preedit, &sel, ink_app_cached.unwrap_or(false), ink_committed_width);
            }
            AppEvent::RedrawRequested => {
                let was_dirty = tabs
                    .active_tab()
                    .map_or(false, |t| t.dirty.swap(false, Ordering::Relaxed));
                let preedit_changed = preedit != prev_preedit;
                if preedit_changed {
                    prev_preedit = preedit.clone();
                }
                render_with_tabs(&mut drawer, &tabs, &preedit, &sel, ink_app_cached.unwrap_or(false), ink_committed_width);
                if was_dirty || preedit_changed {
                    if let Some(ref path) = grid_dump_path {
                        let dump_file = std::path::Path::new(path);
                        if dump_file.exists() {
                            continue;
                        }
                        if let Some(tab) = tabs.active_tab_mut() {
                            let state = tab.terminal.lock().unwrap();
                            let has_content =
                                state
                                    .grid
                                    .cells()
                                    .iter()
                                    .any(|row: &Vec<growterm_types::Cell>| {
                                        row.iter()
                                            .any(|c| c.character != '\0' && c.character != ' ')
                                    });
                            if has_content {
                                let (crow, ccol) = state.grid.cursor_pos();
                                let mut dump = format!("cursor:{crow},{ccol}\ngrid:\n");
                                for (row_idx, row) in state.grid.cells().iter().enumerate() {
                                    let mut text: String = row
                                        .iter()
                                        .map(|c: &growterm_types::Cell| c.character)
                                        .collect();
                                    // Overlay preedit at cursor position
                                    if !preedit.is_empty() && row_idx == crow as usize {
                                        let col = ccol as usize;
                                        let mut chars: Vec<char> = text.chars().collect();
                                        for (j, pc) in preedit.chars().enumerate() {
                                            let pos = col + j;
                                            while chars.len() <= pos {
                                                chars.push(' ');
                                            }
                                            chars[pos] = pc;
                                        }
                                        text = chars.into_iter().collect();
                                    }
                                    dump.push_str(
                                        text.trim_end_matches(|c: char| c == '\0' || c == ' '),
                                    );
                                    dump.push('\n');
                                }
                                drop(state);
                                if let Some(ref dropped_path) = test_dropped_path {
                                    if !test_drop_sent && !dropped_path.is_empty() {
                                        test_drop_sent = true;
                                        deferred =
                                            Some(AppEvent::FileDropped(vec![dropped_path.clone()]));
                                        continue;
                                    }
                                }
                                if let Some(ref input) = test_input {
                                    if !test_input_sent {
                                        let _ = tab.pty_writer.write_all(input.as_bytes());
                                        let _ = tab.pty_writer.flush();
                                        test_input_sent = true;
                                        continue;
                                    }
                                }
                                let _ = std::fs::write(path, &dump);
                            }
                        }
                    }
                }
            }
            AppEvent::FileDropped(paths) => {
                if let Some(tab) = tabs.active_tab_mut() {
                    let text = paths
                        .iter()
                        .map(|p| shell_escape(p))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let _ = tab.pty_writer.write_all(text.as_bytes());
                    let _ = tab.pty_writer.flush();
                }
            }
            AppEvent::CloseRequested => {
                std::process::exit(0);
            }
        }
    }
}

/// grid에서 두 separator(─) 사이에 있는 ❯ 프롬프트 행을 찾음.
fn find_prompt_row(cells: &[Vec<growterm_types::Cell>]) -> Option<usize> {
    let is_separator = |row: &[growterm_types::Cell]| -> bool {
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

fn shell_escape(path: &str) -> String {
    if path.contains(|c: char| c.is_whitespace() || "\"'\\$`!#&|;(){}[]<>?*~".contains(c)) {
        format!("'{}'", path.replace('\'', "'\\''"))
    } else {
        path.to_string()
    }
}

fn render_with_tabs(drawer: &mut GpuDrawer, tabs: &TabManager, preedit: &str, sel: &Selection, is_ink_app: bool, ink_committed_width: u16) {
    let tab = match tabs.active_tab() {
        Some(t) => t,
        None => return,
    };

    let state = tab.terminal.lock().unwrap();
    let scrolled = state.grid.scroll_offset() > 0;
    let cursor = if scrolled {
        None
    } else {
        Some(state.grid.cursor_pos())
    };
    let preedit_str = if preedit.is_empty() || scrolled {
        None
    } else {
        Some(preedit)
    };

    let scrollback_len = state.grid.scrollback_len();
    let rows = state.grid.cells().len();
    let scroll_offset = state.grid.scroll_offset();
    let scrollbar = if scrollback_len > 0 {
        let total = (scrollback_len + rows) as f32;
        let thumb_height = rows as f32 / total;
        let thumb_top = (scrollback_len - scroll_offset) as f32 / total;
        Some((thumb_top, thumb_height))
    } else {
        None
    };
    let visible = state.grid.visible_cells();
    let view_base = (state
        .grid
        .scrollback_len()
        .saturating_sub(state.grid.scroll_offset())) as u32;
    let visible_rows = visible.len() as u16;
    let sel_range = sel.screen_normalized(view_base, visible_rows);

    let show_tab_bar = tabs.show_tab_bar();
    let (cell_w, cell_h) = drawer.cell_size();
    let row_offset = if show_tab_bar { 1 } else { 0 };
    let preedit_pos_override = if is_ink_app && preedit_str.is_some() {
        find_prompt_row(&visible).map(|prompt_row| {
            let col = 2 + ink_committed_width;
            let cols_per_row = visible.first().map_or(80, |r| r.len()) as u16;
            let row = prompt_row as u16 + col / cols_per_row;
            let col = col % cols_per_row;
            eprintln!("  PREEDIT OVERRIDE pos=({},{}) committed_w={}", row, col, ink_committed_width);
            (row, col)
        })
    } else {
        None
    };
    let commands = growterm_render_cmd::generate_with_offset(
        &visible,
        cursor,
        preedit_str,
        sel_range,
        row_offset,
        state.palette,
        preedit_pos_override,
    );
    drop(state);

    let tab_bar = if show_tab_bar {
        Some(TabBarInfo {
            titles: tabs.tab_bar_info().titles,
            active_index: tabs.tab_bar_info().active_index,
            cell_h,
            cell_w,
        })
    } else {
        None
    };

    drawer.draw(&commands, scrollbar, tab_bar.as_ref());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_escape_plain_path() {
        assert_eq!(shell_escape("/Users/me/file.txt"), "/Users/me/file.txt");
    }

    #[test]
    fn shell_escape_path_with_spaces() {
        assert_eq!(
            shell_escape("/Users/me/my file.txt"),
            "'/Users/me/my file.txt'"
        );
    }

    #[test]
    fn shell_escape_path_with_special_chars() {
        assert_eq!(shell_escape("/tmp/a&b.txt"), "'/tmp/a&b.txt'");
    }

    #[test]
    fn shell_escape_path_with_single_quote() {
        assert_eq!(shell_escape("/tmp/it's.txt"), "'/tmp/it'\\''s.txt'");
    }
}
