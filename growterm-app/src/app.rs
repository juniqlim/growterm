use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use growterm_gpu_draw::GpuDrawer;
use growterm_grid::Grid;
use growterm_macos::{AppEvent, MacWindow, Modifiers};
use growterm_vt_parser::VtParser;

use crate::selection::{self, Selection};
use crate::zoom;

struct TerminalState {
    grid: Grid,
    vt_parser: VtParser,
}

pub fn run(window: Arc<MacWindow>, rx: mpsc::Receiver<AppEvent>, mut drawer: GpuDrawer) {
    let (cell_w, cell_h) = drawer.cell_size();
    let mut font_size = crate::FONT_SIZE;
    let (width, height) = window.inner_size();

    let (cols, rows) = zoom::calc_grid_size(width, height, cell_w, cell_h);

    let grid = Grid::new(cols, rows);
    let vt_parser = VtParser::new();
    let terminal = Arc::new(Mutex::new(TerminalState { grid, vt_parser }));
    let dirty = Arc::new(AtomicBool::new(false));

    let mut pty_writer = match growterm_pty::spawn(rows, cols) {
        Ok((reader, writer)) => {
            start_io_thread(reader, Arc::clone(&terminal), Arc::clone(&dirty), window.clone());
            writer
        }
        Err(e) => {
            eprintln!("Failed to spawn PTY: {e}");
            return;
        }
    };

    let mut preedit = String::new();
    let mut sel = Selection::default();
    let mut scroll_accum: f64 = 0.0;
    let grid_dump_path = std::env::var("GROWTERM_GRID_DUMP").ok();
    let test_input = std::env::var("GROWTERM_TEST_INPUT").ok();
    let mut test_input_sent = false;

    for event in rx {
        match event {
            AppEvent::TextCommit(text) => {
                preedit.clear();
                let _ = pty_writer.write_all(text.as_bytes());
                let _ = pty_writer.flush();
            }
            AppEvent::Preedit(text) => {
                preedit = text;
                window.request_redraw();
            }
            AppEvent::KeyInput { keycode, characters, modifiers } => {
                // Cmd+= / Cmd+- (zoom), Cmd+V (paste), Cmd+PageUp/Down (scroll)
                if modifiers.contains(Modifiers::SUPER) {
                    if keycode == growterm_macos::key_convert::keycode::PAGE_UP
                        || keycode == growterm_macos::key_convert::keycode::PAGE_DOWN
                    {
                        let mut state = terminal.lock().unwrap();
                        let row_count = state.grid.cells().len();
                        if keycode == growterm_macos::key_convert::keycode::PAGE_UP {
                            state.grid.scroll_up_view(row_count);
                        } else {
                            state.grid.scroll_down_view(row_count);
                        }
                        drop(state);
                        render(&mut drawer, &terminal, &preedit, &sel);
                        continue;
                    }
                    // Cmd+V paste (keycode 비교: 한글 IME에서도 동작)
                    if keycode == growterm_macos::key_convert::keycode::ANSI_V {
                        if let Ok(mut clipboard) = arboard::Clipboard::new() {
                            if let Ok(text) = clipboard.get_text() {
                                if !text.is_empty() {
                                    let _ = pty_writer.write_all(text.as_bytes());
                                    let _ = pty_writer.flush();
                                }
                            }
                        }
                        continue;
                    }
                    // Cmd+= / Cmd+- (zoom, keycode 비교: 한글 IME에서도 동작)
                    let zoom_delta = match keycode {
                        kc if kc == growterm_macos::key_convert::keycode::ANSI_EQUAL => Some(2.0f32),
                        kc if kc == growterm_macos::key_convert::keycode::ANSI_MINUS => Some(-2.0f32),
                        _ => None,
                    };
                    if let Some(delta) = zoom_delta {
                        font_size = zoom::apply_zoom(font_size, delta);
                        drawer.set_font_size(font_size);
                        let (cw, ch) = drawer.cell_size();
                        let (w, h) = window.inner_size();
                        let (cols, rows) = zoom::calc_grid_size(w, h, cw, ch);
                        let mut state = terminal.lock().unwrap();
                        state.grid.resize(cols, rows);
                        drop(state);
                        let _ = pty_writer.resize(rows, cols);
                        render(&mut drawer, &terminal, &preedit, &sel);
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
                    let _ = pty_writer.write_all(&bytes);
                    let _ = pty_writer.flush();
                }
            }
            AppEvent::MouseDown(x, y) => {
                let (cw, ch) = drawer.cell_size();
                let (row, col) = selection::pixel_to_cell(x as f32, y as f32, cw, ch);
                sel.begin(row, col);
                window.request_redraw();
            }
            AppEvent::MouseDragged(x, y) => {
                if sel.active {
                    let (cw, ch) = drawer.cell_size();
                    let (row, col) = selection::pixel_to_cell(x as f32, y as f32, cw, ch);
                    sel.update(row, col);
                    window.request_redraw();
                }
            }
            AppEvent::MouseUp(x, y) => {
                let (cw, ch) = drawer.cell_size();
                let (row, col) = selection::pixel_to_cell(x as f32, y as f32, cw, ch);
                sel.update(row, col);
                sel.finish();
                if !sel.is_empty() {
                    let state = terminal.lock().unwrap();
                    let text = selection::extract_text(state.grid.cells(), &sel);
                    drop(state);
                    if !text.is_empty() {
                        if let Ok(mut clipboard) = arboard::Clipboard::new() {
                            let _ = clipboard.set_text(text);
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
                    let mut state = terminal.lock().unwrap();
                    if lines > 0 {
                        state.grid.scroll_up_view(lines as usize);
                    } else {
                        state.grid.scroll_down_view((-lines) as usize);
                    }
                    drop(state);
                    render(&mut drawer, &terminal, &preedit, &sel);
                }
            }
            AppEvent::Resize(w, h) => {
                drawer.resize(w, h);
                let (cw, ch) = drawer.cell_size();
                let (cols, rows) = zoom::calc_grid_size(w, h, cw, ch);
                terminal.lock().unwrap().grid.resize(cols, rows);
                let _ = pty_writer.resize(rows, cols);
                render(&mut drawer, &terminal, &preedit, &sel);
            }
            AppEvent::RedrawRequested => {
                let was_dirty = dirty.swap(false, Ordering::Relaxed);
                render(&mut drawer, &terminal, &preedit, &sel);
                if was_dirty {
                    if let Some(ref path) = grid_dump_path {
                        let dump_file = std::path::Path::new(path);
                        if dump_file.exists() {
                            continue;
                        }
                        let state = terminal.lock().unwrap();
                        let has_content = state.grid.cells().iter().any(|row| {
                            row.iter().any(|c| c.character != '\0' && c.character != ' ')
                        });
                        if has_content {
                            let (crow, ccol) = state.grid.cursor_pos();
                            let mut dump = format!("cursor:{crow},{ccol}\ngrid:\n");
                            for row in state.grid.cells() {
                                let text: String = row.iter().map(|c| c.character).collect();
                                dump.push_str(text.trim_end_matches(|c: char| c == '\0' || c == ' '));
                                dump.push('\n');
                            }
                            drop(state);
                            // If test_input is set and not yet sent, send it and wait for next dirty render.
                            if let Some(ref input) = test_input {
                                if !test_input_sent {
                                    let _ = pty_writer.write_all(input.as_bytes());
                                    let _ = pty_writer.flush();
                                    test_input_sent = true;
                                    // Don't dump yet — wait for the command output.
                                    continue;
                                }
                            }
                            let _ = std::fs::write(path, &dump);
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

fn render(drawer: &mut GpuDrawer, terminal: &Arc<Mutex<TerminalState>>, preedit: &str, sel: &Selection) {
    let state = terminal.lock().unwrap();
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
    let commands = growterm_render_cmd::generate(&visible, cursor, preedit_str, sel_range);
    drop(state);
    drawer.draw(&commands, scrollbar);
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
