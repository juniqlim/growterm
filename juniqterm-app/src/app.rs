use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use winit::application::ApplicationHandler;
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

use juniqterm_gpu_draw::GpuDrawer;
use juniqterm_grid::Grid;
use juniqterm_pty::PtyWriter;
use juniqterm_vt_parser::VtParser;

use crate::event_action::{self, Action, ImeHandler};
use crate::key_convert::convert_key;
use crate::selection::{self, Selection};
use crate::zoom;

const FONT_SIZE: f32 = 32.0;

pub struct TerminalState {
    pub grid: Grid,
    pub vt_parser: VtParser,
}

pub struct App {
    proxy: EventLoopProxy<()>,
    window: Option<Arc<Window>>,
    drawer: Option<GpuDrawer>,
    pty_writer: Option<PtyWriter>,
    terminal: Option<Arc<Mutex<TerminalState>>>,
    dirty: Arc<AtomicBool>,
    modifiers: ModifiersState,
    font_size: f32,
    ime: ImeHandler,
    selection: Selection,
    cell_size: (f32, f32),
    cursor_pos_px: (f64, f64),
    mouse_pressed: bool,
}

impl App {
    pub fn new(proxy: EventLoopProxy<()>) -> Self {
        Self {
            proxy,
            window: None,
            drawer: None,
            pty_writer: None,
            terminal: None,
            dirty: Arc::new(AtomicBool::new(false)),
            modifiers: ModifiersState::empty(),
            font_size: FONT_SIZE,
            ime: ImeHandler::new(),
            selection: Selection::default(),
            cell_size: (0.0, 0.0),
            cursor_pos_px: (0.0, 0.0),
            mouse_pressed: false,
        }
    }

    fn start_io_thread(
        mut reader: juniqterm_pty::PtyReader,
        terminal: Arc<Mutex<TerminalState>>,
        dirty: Arc<AtomicBool>,
        proxy: EventLoopProxy<()>,
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
                        let _ = proxy.send_event(());
                    }
                    Err(e) => {
                        // EIO on macOS means shell process exited
                        if e.raw_os_error() == Some(libc::EIO) {
                            break;
                        }
                        // Other errors: also exit
                        break;
                    }
                }
            }
            // Request one last redraw + exit
            let _ = proxy.send_event(());
        });
    }

    fn write_pty(&mut self, bytes: &[u8]) {
        if let Some(writer) = &mut self.pty_writer {
            let _ = writer.write_all(bytes);
            let _ = writer.flush();
        }
    }

    fn render(&mut self) {
        if let (Some(drawer), Some(terminal)) = (&mut self.drawer, &self.terminal) {
            let state = terminal.lock().unwrap();
            let scrolled = state.grid.scroll_offset() > 0;
            let cursor = if scrolled { None } else { Some(state.grid.cursor_pos()) };
            let preedit = if self.ime.preedit().is_empty() || scrolled {
                None
            } else {
                Some(self.ime.preedit())
            };
            let sel = if !self.selection.is_empty() {
                let n = self.selection.normalized();
                Some((n.0, n.1))
            } else {
                None
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
            let commands =
                juniqterm_render_cmd::generate(&visible, cursor, preedit, sel);
            drop(state);
            drawer.draw(&commands, scrollbar);
        }
    }
}

impl ApplicationHandler<()> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title("juniqterm")
            .with_inner_size(winit::dpi::LogicalSize::new(800, 600));
        let window = Arc::new(event_loop.create_window(attrs).unwrap());

        let mut drawer = GpuDrawer::new(window.clone(), FONT_SIZE);
        let (cell_w, cell_h) = drawer.cell_size();
        self.cell_size = (cell_w, cell_h);
        let size = window.inner_size();
        let scale = window.scale_factor();
        eprintln!("[DEBUG] scale_factor={scale}, inner_size={}x{}, cell=({cell_w}, {cell_h})", size.width, size.height);
        drawer.resize(size.width, size.height);

        let (cols, rows) = zoom::calc_grid_size(
            size.width, size.height, cell_w, cell_h,
        );

        let grid = Grid::new(cols, rows);
        let vt_parser = VtParser::new();
        let terminal = Arc::new(Mutex::new(TerminalState { grid, vt_parser }));

        match juniqterm_pty::spawn(rows, cols) {
            Ok((reader, writer)) => {
                Self::start_io_thread(
                    reader,
                    Arc::clone(&terminal),
                    Arc::clone(&self.dirty),
                    self.proxy.clone(),
                );
                self.pty_writer = Some(writer);
            }
            Err(e) => {
                eprintln!("Failed to spawn PTY: {e}");
                event_loop.exit();
                return;
            }
        }

        self.terminal = Some(terminal);
        self.drawer = Some(drawer);
        window.set_ime_allowed(true);
        window.set_ime_cursor_area(
            winit::dpi::PhysicalPosition::new(0, 0),
            winit::dpi::PhysicalSize::new(0, 0),
        );
        self.window = Some(window);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: ()) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::ModifiersChanged(new_modifiers) => {
                self.modifiers = new_modifiers.state();
            }
            WindowEvent::Ime(ime) => match ime {
                Ime::Preedit(text, _) => {
                    eprintln!("[IME] Preedit: {:?}", text);
                    match self.ime.handle_ime_preedit(&text) {
                        Action::SetPreedit(_) => {}
                        Action::WritePty(bytes) => self.write_pty(&bytes),
                    }
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
                Ime::Commit(text) => {
                    eprintln!("[IME] Commit: {:?}", text);
                    match self.ime.handle_ime_commit(&text) {
                        Action::WritePty(bytes) => self.write_pty(&bytes),
                        _ => {}
                    }
                }
                Ime::Enabled => {
                    eprintln!("[IME] Enabled");
                }
                Ime::Disabled => {
                    eprintln!("[IME] Disabled");
                }
            },
            WindowEvent::KeyboardInput { event, .. } => {
                // Cmd+=/- for font size zoom
                if self.modifiers.super_key()
                    && event.state == winit::event::ElementState::Pressed
                {
                    let is_v = matches!(
                        event.physical_key,
                        winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyV)
                    );
                    let key_str = match &event.logical_key {
                        winit::keyboard::Key::Character(s) => Some(s.as_str()),
                        _ => None,
                    };
                    let is_page_up = matches!(
                        event.physical_key,
                        winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::PageUp)
                    );
                    let is_page_down = matches!(
                        event.physical_key,
                        winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::PageDown)
                    );
                    if is_page_up || is_page_down {
                        if let Some(terminal) = &self.terminal {
                            let mut state = terminal.lock().unwrap();
                            let rows = state.grid.cells().len();
                            if is_page_up {
                                state.grid.scroll_up_view(rows);
                            } else {
                                state.grid.scroll_down_view(rows);
                            }
                        }
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                        return;
                    }
                    if is_v {
                        if let Ok(mut clipboard) = arboard::Clipboard::new() {
                            if let Ok(text) = clipboard.get_text() {
                                if !text.is_empty() {
                                    self.write_pty(text.as_bytes());
                                }
                            }
                        }
                        return;
                    }
                    let delta = key_str.and_then(zoom::zoom_delta);
                    if let Some(delta) = delta {
                        self.font_size = zoom::apply_zoom(self.font_size, delta);
                        if let (Some(drawer), Some(window)) =
                            (&mut self.drawer, &self.window)
                        {
                            drawer.set_font_size(self.font_size);
                            let (cell_w, cell_h) = drawer.cell_size();
                            self.cell_size = (cell_w, cell_h);
                            let size = window.inner_size();
                            let (cols, rows) = zoom::calc_grid_size(
                                size.width, size.height, cell_w, cell_h,
                            );
                            if let Some(terminal) = &self.terminal {
                                terminal.lock().unwrap().grid.resize(cols, rows);
                            }
                            if let Some(writer) = &self.pty_writer {
                                let _ = writer.resize(rows, cols);
                            }
                            window.request_redraw();
                        }
                        return;
                    }
                    return;
                }

                if event.state != winit::event::ElementState::Pressed {
                    return;
                }

                let is_plain_char = matches!(
                    &event.logical_key,
                    winit::keyboard::Key::Character(_)
                ) && !self.modifiers.control_key()
                    && !self.modifiers.alt_key();

                eprintln!(
                    "[KEY] logical={:?} text={:?} is_plain_char={} preedit={:?}",
                    event.logical_key, event.text, is_plain_char, self.ime.preedit()
                );

                if is_plain_char {
                    if let Some(action) = self.ime.handle_plain_char_input(
                        event.text.as_ref().map(|t| t.as_str()),
                    ) {
                        match action {
                            Action::WritePty(bytes) => self.write_pty(&bytes),
                            _ => {}
                        }
                    }
                    return;
                }
                if let Some(key_event) =
                    convert_key(&event.logical_key, event.state, self.modifiers)
                {
                    match event_action::handle_keyboard_input(key_event) {
                        Action::WritePty(bytes) => self.write_pty(&bytes),
                        _ => {}
                    }
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(drawer) = &mut self.drawer {
                    drawer.resize(size.width, size.height);
                    let (cell_w, cell_h) = drawer.cell_size();
                    let (cols, rows) = zoom::calc_grid_size(
                        size.width, size.height, cell_w, cell_h,
                    );

                    if let Some(terminal) = &self.terminal {
                        terminal.lock().unwrap().grid.resize(cols, rows);
                    }
                    if let Some(writer) = &self.pty_writer {
                        let _ = writer.resize(rows, cols);
                    }
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                self.dirty.swap(false, Ordering::Relaxed);
                self.render();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos_px = (position.x, position.y);
                if self.mouse_pressed {
                    let (row, col) = selection::pixel_to_cell(
                        position.x as f32,
                        position.y as f32,
                        self.cell_size.0,
                        self.cell_size.1,
                    );
                    self.selection.update(row, col);
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => {
                        if y > 0.0 { 3i32 } else if y < 0.0 { -3 } else { 0 }
                    }
                    MouseScrollDelta::PixelDelta(pos) => {
                        let cell_h = self.cell_size.1 as f64;
                        if cell_h > 0.0 { (pos.y / cell_h).round() as i32 } else { 0 }
                    }
                };
                if lines != 0 {
                    if let Some(terminal) = &self.terminal {
                        let mut state = terminal.lock().unwrap();
                        if lines > 0 {
                            state.grid.scroll_up_view(lines as usize);
                        } else {
                            state.grid.scroll_down_view((-lines) as usize);
                        }
                    }
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                match state {
                    ElementState::Pressed => {
                        self.mouse_pressed = true;
                        let (row, col) = selection::pixel_to_cell(
                            self.cursor_pos_px.0 as f32,
                            self.cursor_pos_px.1 as f32,
                            self.cell_size.0,
                            self.cell_size.1,
                        );
                        self.selection.begin(row, col);
                    }
                    ElementState::Released => {
                        self.mouse_pressed = false;
                        self.selection.finish();
                        if !self.selection.is_empty() {
                            if let Some(terminal) = &self.terminal {
                                let state = terminal.lock().unwrap();
                                let text = selection::extract_text(
                                    state.grid.cells(),
                                    &self.selection,
                                );
                                drop(state);
                                if !text.is_empty() {
                                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                        let _ = clipboard.set_text(text);
                                    }
                                }
                            }
                        }
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
