use std::io::Write;
use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc};

use growterm_gpu_draw::{GpuDrawer, TabBarInfo};
use growterm_macos::{AppEvent, MacWindow, Modifiers};

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
    let mut sel = Selection::default();
    let mut scroll_accum: f64 = 0.0;
    let mut deferred: Option<AppEvent> = None;
    let grid_dump_path = std::env::var("GROWTERM_GRID_DUMP").ok();
    let test_input = std::env::var("GROWTERM_TEST_INPUT").ok();
    let mut test_input_sent = false;

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
                if let Some(tab) = tabs.active_tab_mut() {
                    let _ = tab.pty_writer.write_all(text.as_bytes());
                    let _ = tab.pty_writer.flush();
                }
            }
            AppEvent::Preedit(text) => {
                preedit = text;
                window.request_redraw();
            }
            AppEvent::KeyInput { keycode, characters, modifiers } => {
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
                        render_with_tabs(&mut drawer, &tabs, &preedit, &sel);
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
                        render_with_tabs(&mut drawer, &tabs, &preedit, &sel);
                        continue;
                    }

                    // Cmd+Shift+[ / Cmd+Shift+]: prev/next tab
                    if modifiers.contains(Modifiers::SHIFT) {
                        if keycode == kc::ANSI_LEFT_BRACKET {
                            tabs.prev_tab();
                            sel.clear();
                            preedit.clear();
                            render_with_tabs(&mut drawer, &tabs, &preedit, &sel);
                            continue;
                        }
                        if keycode == kc::ANSI_RIGHT_BRACKET {
                            tabs.next_tab();
                            sel.clear();
                            preedit.clear();
                            render_with_tabs(&mut drawer, &tabs, &preedit, &sel);
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
                            render_with_tabs(&mut drawer, &tabs, &preedit, &sel);
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
                        render_with_tabs(&mut drawer, &tabs, &preedit, &sel);
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
                        render_with_tabs(&mut drawer, &tabs, &preedit, &sel);
                        continue;
                    }
                    continue;
                }

                if let Some(key_event) = growterm_macos::convert_key(
                    keycode,
                    characters.as_deref(),
                    modifiers,
                ) {
                    let bytes = growterm_input::encode(key_event);
                    if let Some(tab) = tabs.active_tab_mut() {
                        let _ = tab.pty_writer.write_all(&bytes);
                        let _ = tab.pty_writer.flush();
                    }
                }
            }
            AppEvent::MouseDown(x, y) => {
                let (cw, ch) = drawer.cell_size();
                let (row, col) = selection::pixel_to_cell(x as f32, y as f32 - tabs.mouse_y_offset(ch), cw, ch);
                sel.begin(row, col);
                window.request_redraw();
            }
            AppEvent::MouseDragged(x, y) => {
                if sel.active {
                    let (cw, ch) = drawer.cell_size();
                    let (row, col) = selection::pixel_to_cell(x as f32, y as f32 - tabs.mouse_y_offset(ch), cw, ch);
                    sel.update(row, col);
                    window.request_redraw();
                }
            }
            AppEvent::MouseUp(x, y) => {
                let (cw, ch) = drawer.cell_size();
                let (row, col) = selection::pixel_to_cell(x as f32, y as f32 - tabs.mouse_y_offset(ch), cw, ch);
                sel.update(row, col);
                sel.finish();
                if !sel.is_empty() {
                    if let Some(tab) = tabs.active_tab() {
                        let state = tab.terminal.lock().unwrap();
                        let text = selection::extract_text(state.grid.cells(), &sel);
                        drop(state);
                        if !text.is_empty() {
                            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                let _ = clipboard.set_text(text);
                            }
                        }
                    }
                }
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
                    render_with_tabs(&mut drawer, &tabs, &preedit, &sel);
                }
            }
            AppEvent::Resize(mut w, mut h) => {
                loop {
                    match rx.try_recv() {
                        Ok(AppEvent::Resize(nw, nh)) => { w = nw; h = nh; }
                        Ok(other) => { deferred = Some(other); break; }
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
                render_with_tabs(&mut drawer, &tabs, &preedit, &sel);
            }
            AppEvent::RedrawRequested => {
                let was_dirty = tabs.active_tab().map_or(false, |t| t.dirty.swap(false, Ordering::Relaxed));
                render_with_tabs(&mut drawer, &tabs, &preedit, &sel);
                if was_dirty {
                    if let Some(ref path) = grid_dump_path {
                        let dump_file = std::path::Path::new(path);
                        if dump_file.exists() {
                            continue;
                        }
                        if let Some(tab) = tabs.active_tab_mut() {
                            let state = tab.terminal.lock().unwrap();
                            let has_content = state.grid.cells().iter().any(|row: &Vec<growterm_types::Cell>| {
                                row.iter().any(|c| c.character != '\0' && c.character != ' ')
                            });
                            if has_content {
                                let (crow, ccol) = state.grid.cursor_pos();
                                let mut dump = format!("cursor:{crow},{ccol}\ngrid:\n");
                                for row in state.grid.cells() {
                                    let text: String = row.iter().map(|c: &growterm_types::Cell| c.character).collect();
                                    dump.push_str(text.trim_end_matches(|c: char| c == '\0' || c == ' '));
                                    dump.push('\n');
                                }
                                drop(state);
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
            AppEvent::CloseRequested => {
                std::process::exit(0);
            }
        }
    }
}

fn render_with_tabs(drawer: &mut GpuDrawer, tabs: &TabManager, preedit: &str, sel: &Selection) {
    let tab = match tabs.active_tab() {
        Some(t) => t,
        None => return,
    };

    let state = tab.terminal.lock().unwrap();
    let scrolled = state.grid.scroll_offset() > 0;
    let cursor = if scrolled { None } else { Some(state.grid.cursor_pos()) };
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
    let sel_range = if !sel.is_empty() { Some(sel.normalized()) } else { None };

    let show_tab_bar = tabs.show_tab_bar();
    let (cell_w, cell_h) = drawer.cell_size();
    let row_offset = if show_tab_bar { 1 } else { 0 };
    let commands = growterm_render_cmd::generate_with_offset(&visible, cursor, preedit_str, sel_range, row_offset);
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
