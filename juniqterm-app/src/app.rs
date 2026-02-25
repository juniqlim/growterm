use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use juniqterm_gpu_draw::GpuDrawer;
use juniqterm_grid::Grid;
use juniqterm_macos::{AppEvent, MacWindow, Modifiers};
use juniqterm_vt_parser::VtParser;

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

    let mut pty_writer = match juniqterm_pty::spawn(rows, cols) {
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
    let grid_dump_path = std::env::var("JUNIQTERM_GRID_DUMP").ok();
    let test_input = std::env::var("JUNIQTERM_TEST_INPUT").ok();
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
                    if keycode == juniqterm_macos::key_convert::keycode::PAGE_UP
                        || keycode == juniqterm_macos::key_convert::keycode::PAGE_DOWN
                    {
                        let mut state = terminal.lock().unwrap();
                        let row_count = state.grid.cells().len();
                        if keycode == juniqterm_macos::key_convert::keycode::PAGE_UP {
                            state.grid.scroll_up_view(row_count);
                        } else {
                            state.grid.scroll_down_view(row_count);
                        }
                        drop(state);
                        render(&mut drawer, &terminal, &preedit);
                        continue;
                    }
                    // Cmd+V paste
                    if let Some(ref chars) = characters {
                        if chars == "v" || chars == "V" {
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
                    }
                    // Cmd+= / Cmd+-
                    if let Some(ref chars) = characters {
                        if let Some(delta) = zoom::zoom_delta(chars) {
                            font_size = zoom::apply_zoom(font_size, delta);
                            drawer.set_font_size(font_size);
                            let (cw, ch) = drawer.cell_size();
                            let (w, h) = window.inner_size();
                            let (cols, rows) = zoom::calc_grid_size(w, h, cw, ch);
                            let mut state = terminal.lock().unwrap();
                            state.grid.resize(cols, rows);
                            drop(state);
                            let _ = pty_writer.resize(rows, cols);
                            render(&mut drawer, &terminal, &preedit);
                            continue;
                        }
                    }
                    continue;
                }

                if let Some(key_event) = juniqterm_macos::convert_key(
                    keycode,
                    characters.as_deref(),
                    modifiers,
                ) {
                    let bytes = juniqterm_input::encode(key_event);
                    let _ = pty_writer.write_all(&bytes);
                    let _ = pty_writer.flush();
                }
            }
            AppEvent::Resize(w, h) => {
                drawer.resize(w, h);
                let (cw, ch) = drawer.cell_size();
                let (cols, rows) = zoom::calc_grid_size(w, h, cw, ch);
                terminal.lock().unwrap().grid.resize(cols, rows);
                let _ = pty_writer.resize(rows, cols);
                render(&mut drawer, &terminal, &preedit);
            }
            AppEvent::RedrawRequested => {
                let was_dirty = dirty.swap(false, Ordering::Relaxed);
                render(&mut drawer, &terminal, &preedit);
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

fn render(drawer: &mut GpuDrawer, terminal: &Arc<Mutex<TerminalState>>, preedit: &str) {
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
    let commands = juniqterm_render_cmd::generate(&visible, cursor, preedit_str, None);
    drop(state);
    drawer.draw(&commands, scrollbar);
}

fn start_io_thread(
    mut reader: juniqterm_pty::PtyReader,
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
