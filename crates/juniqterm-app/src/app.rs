use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

use juniqterm_gpu_draw::GpuDrawer;
use juniqterm_grid::Grid;
use juniqterm_pty::PtyWriter;
use juniqterm_vt_parser::VtParser;

use crate::key_convert::convert_key;

const FONT_SIZE: f32 = 24.0;

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
        let size = window.inner_size();
        drawer.resize(size.width, size.height);

        let cols = (size.width as f32 / cell_w).floor() as u16;
        let rows = (size.height as f32 / cell_h).floor() as u16;
        let cols = cols.max(1);
        let rows = rows.max(1);

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
            WindowEvent::KeyboardInput { event, .. } => {
                // Cmd+=/- for font size zoom
                if self.modifiers.super_key()
                    && event.state == winit::event::ElementState::Pressed
                {
                    let zoom = match &event.logical_key {
                        winit::keyboard::Key::Character(s) if s.as_str() == "=" || s.as_str() == "+" => Some(2.0f32),
                        winit::keyboard::Key::Character(s) if s.as_str() == "-" => Some(-2.0f32),
                        _ => None,
                    };
                    if let Some(delta) = zoom {
                        self.font_size = (self.font_size + delta).clamp(8.0, 72.0);
                        if let (Some(drawer), Some(window)) =
                            (&mut self.drawer, &self.window)
                        {
                            drawer.set_font_size(self.font_size);
                            let (cell_w, cell_h) = drawer.cell_size();
                            let size = window.inner_size();
                            let cols = (size.width as f32 / cell_w).floor() as u16;
                            let rows = (size.height as f32 / cell_h).floor() as u16;
                            let cols = cols.max(1);
                            let rows = rows.max(1);
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
                }

                if let Some(key_event) =
                    convert_key(&event.logical_key, event.state, self.modifiers)
                {
                    let bytes = juniqterm_input::encode(key_event);
                    if let Some(writer) = &mut self.pty_writer {
                        let _ = writer.write_all(&bytes);
                        let _ = writer.flush();
                    }
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(drawer) = &mut self.drawer {
                    drawer.resize(size.width, size.height);
                    let (cell_w, cell_h) = drawer.cell_size();
                    let cols = (size.width as f32 / cell_w).floor() as u16;
                    let rows = (size.height as f32 / cell_h).floor() as u16;
                    let cols = cols.max(1);
                    let rows = rows.max(1);

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
                if !self.dirty.swap(false, Ordering::Relaxed) {
                    // Still draw on first frame or resize
                    if let (Some(drawer), Some(terminal)) =
                        (&mut self.drawer, &self.terminal)
                    {
                        let state = terminal.lock().unwrap();
                        let cursor = state.grid.cursor_pos();
                        let commands = juniqterm_render_cmd::generate(state.grid.cells(), Some(cursor));
                        drop(state);
                        drawer.draw(&commands);
                    }
                    return;
                }
                if let (Some(drawer), Some(terminal)) = (&mut self.drawer, &self.terminal)
                {
                    let state = terminal.lock().unwrap();
                    let cursor = state.grid.cursor_pos();
                    let commands = juniqterm_render_cmd::generate(state.grid.cells(), Some(cursor));
                    drop(state);
                    drawer.draw(&commands);
                }
            }
            _ => {}
        }
    }
}
