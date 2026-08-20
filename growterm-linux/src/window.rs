use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{ModifiersState, PhysicalKey};
use winit::window::{Theme, Window, WindowAttributes, WindowId};

use crate::event::{AppEvent, Modifiers};
use crate::key_convert::physical_keycode_to_app_keycode;

pub struct MacWindow {
    window: Window,
    copy_mode: AtomicBool,
    has_selection: AtomicBool,
    sender: std::sync::OnceLock<mpsc::Sender<AppEvent>>,
}

impl MacWindow {
    fn new(window: Window) -> Self {
        Self {
            window,
            copy_mode: AtomicBool::new(false),
            has_selection: AtomicBool::new(false),
            sender: std::sync::OnceLock::new(),
        }
    }

    /// Reload whenever the config file changes, so editing it is the whole
    /// gesture — no key to remember, and the desktop's menu can just write.
    pub fn watch_config(&self, path: std::path::PathBuf) {
        if let Some(sender) = self.sender.get() {
            crate::config_watch::spawn(path, sender.clone());
        }
    }

    pub fn inner_size(&self) -> (u32, u32) {
        let size = self.window.inner_size();
        (size.width.max(1), size.height.max(1))
    }

    pub fn backing_scale_factor(&self) -> f64 {
        self.window.scale_factor()
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub fn set_title(&self, title: &str) {
        self.window.set_title(title);
    }

    /// Ctrl+C copies here, so it has to know whether copying is what the user
    /// meant — with nothing selected the key stays SIGINT.
    pub fn set_has_selection(&self, has_selection: bool) {
        self.has_selection.store(has_selection, Ordering::Relaxed);
    }

    fn has_selection(&self) -> bool {
        self.has_selection.load(Ordering::Relaxed)
    }

    pub fn set_copy_mode(&self, enabled: bool) {
        self.copy_mode.store(enabled, Ordering::Relaxed);
        // Copy mode uses vim-style navigation (j/k). While a Hangul IME is
        // active it would otherwise intercept those keys as composition, so
        // disable IME in copy mode and restore it on exit.
        self.window.set_ime_allowed(!enabled);
    }

    pub fn set_ime_cursor_rect(&self, rect: Option<(f32, f32, f32, f32)>) {
        if let Some((x, y, w, h)) = rect {
            self.window.set_ime_cursor_area(
                PhysicalPosition::new(x, y),
                PhysicalSize::new(w.max(1.0), h.max(1.0)),
            );
        }
    }

    pub fn discard_marked_text(&self) {}

    pub fn set_pomodoro_checked(&self, _checked: bool) {}

    pub fn set_response_timer_checked(&self, _checked: bool) {}

    pub fn set_coaching_checked(&self, _checked: bool) {}

    pub fn set_coaching_menu_enabled(&self, _enabled: bool) {}

    pub fn set_transparent_tab_bar_checked(&self, _checked: bool) {}

    pub fn set_transparent_mode(&self, _enabled: bool) {}

    pub fn title_bar_height(&self) -> f64 {
        0.0
    }

    pub fn set_position(&self, x: f64, y: f64) {
        self.window
            .set_outer_position(PhysicalPosition::new(x as i32, y as i32));
    }

    pub fn show(&self) {
        self.window.set_visible(true);
    }

    pub fn set_pointing_hand_cursor(&self, _enabled: bool) {}

    fn copy_mode_enabled(&self) -> bool {
        self.copy_mode.load(Ordering::Relaxed)
    }
}

unsafe impl Send for MacWindow {}
unsafe impl Sync for MacWindow {}

impl HasWindowHandle for MacWindow {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        self.window.window_handle()
    }
}

impl HasDisplayHandle for MacWindow {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        self.window.display_handle()
    }
}

struct GrowtermApp<F>
where
    F: FnOnce(Arc<MacWindow>, mpsc::Receiver<AppEvent>) + 'static,
{
    setup: Option<F>,
    window_size: (f64, f64),
    window_position: Option<(f64, f64)>,
    window: Option<Arc<MacWindow>>,
    sender: Option<mpsc::Sender<AppEvent>>,
    modifiers: ModifiersState,
    cursor_position: (f64, f64),
    mouse_left_pressed: bool,
    ime_composing: bool,
}

impl<F> GrowtermApp<F>
where
    F: FnOnce(Arc<MacWindow>, mpsc::Receiver<AppEvent>) + 'static,
{
    fn send(&self, event: AppEvent) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(event);
        }
    }

    fn current_modifiers(&self) -> Modifiers {
        convert_modifiers(self.modifiers)
    }

    fn mouse_modifiers(&self) -> Modifiers {
        as_mouse_modifiers(self.current_modifiers())
    }
}

impl<F> ApplicationHandler for GrowtermApp<F>
where
    F: FnOnce(Arc<MacWindow>, mpsc::Receiver<AppEvent>) + 'static,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let mut attrs = WindowAttributes::default()
            .with_title("growterm")
            .with_theme(Some(Theme::Dark))
            .with_inner_size(LogicalSize::new(self.window_size.0, self.window_size.1))
            .with_visible(false);
        if let Some((x, y)) = self.window_position {
            attrs = attrs.with_position(LogicalPosition::new(x, y));
        }

        let raw_window = event_loop
            .create_window(attrs)
            .expect("create linux window");
        raw_window.set_ime_allowed(true);
        let window = Arc::new(MacWindow::new(raw_window));
        let (tx, rx) = mpsc::channel();
        let _ = window.sender.set(tx.clone());
        self.sender = Some(tx);

        if let Some(setup) = self.setup.take() {
            setup(window.clone(), rx);
        }

        window.show();
        window.request_redraw();
        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.send(AppEvent::CloseRequested);
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                self.send(AppEvent::Resize(size.width.max(1), size.height.max(1)));
            }
            WindowEvent::RedrawRequested => {
                self.send(AppEvent::RedrawRequested);
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::Focused(focused) => {
                self.send(AppEvent::FocusChanged(focused));
            }
            WindowEvent::Ime(ime) => match ime {
                Ime::Enabled | Ime::Disabled => {
                    self.ime_composing = false;
                }
                Ime::Preedit(text, _) => {
                    self.ime_composing = !text.is_empty();
                    self.send(AppEvent::Preedit(text));
                }
                Ime::Commit(text) => {
                    self.ime_composing = false;
                    self.send(AppEvent::Preedit(String::new()));
                    self.send(AppEvent::TextCommit(text));
                }
            },
            WindowEvent::KeyboardInput { event, .. } => {
                let event_type = match event.state {
                    ElementState::Pressed if event.repeat => growterm_types::KeyEventType::Repeat,
                    ElementState::Pressed => growterm_types::KeyEventType::Press,
                    ElementState::Released => growterm_types::KeyEventType::Release,
                };
                let keycode = match event.physical_key {
                    PhysicalKey::Code(code) => physical_keycode_to_app_keycode(code),
                    PhysicalKey::Unidentified(_) => None,
                };
                let characters = event.text.as_ref().map(|text| text.to_string());
                let mut modifiers = self.current_modifiers();

                let has_selection = self
                    .window
                    .as_ref()
                    .map(|window| window.has_selection())
                    .unwrap_or(false);
                let claimed = shortcut(keycode, modifiers, has_selection);

                // A composing IME owns the keyboard — except for these, which it
                // would otherwise swallow for as long as a syllable is unfinished.
                if self.ime_composing && claimed.is_none() {
                    return;
                }

                match claimed {
                    Some(Shortcut::AsSuper(remapped)) => modifiers = remapped,
                    None => {}
                }

                let should_commit_text = event_type == growterm_types::KeyEventType::Press
                    && !self
                        .window
                        .as_ref()
                        .map(|window| window.copy_mode_enabled())
                        .unwrap_or(false)
                    && !modifiers
                        .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER)
                    && characters
                        .as_deref()
                        .map(|text| text.chars().all(|c| !c.is_control()))
                        .unwrap_or(false);

                if should_commit_text {
                    if let Some(text) = characters {
                        self.send(AppEvent::TextCommit(text));
                    }
                    return;
                }

                if let Some(keycode) = keycode {
                    self.send(AppEvent::KeyInput {
                        keycode,
                        characters,
                        modifiers,
                        event_type,
                    });
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = (position.x, position.y);
                // winit has no separate drag event; while the left button is
                // held, report movement as a drag (matches macOS mouseDragged)
                // so tab reorder / selection / scrollbar dragging work.
                if self.mouse_left_pressed {
                    self.send(AppEvent::MouseDragged(position.x, position.y));
                } else {
                    self.send(AppEvent::MouseMoved(
                        position.x,
                        position.y,
                        self.mouse_modifiers(),
                    ));
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left {
                    let (x, y) = self.cursor_position;
                    match state {
                        ElementState::Pressed => {
                            self.mouse_left_pressed = true;
                            self.send(AppEvent::MouseDown(x, y, self.mouse_modifiers()));
                        }
                        ElementState::Released => {
                            self.mouse_left_pressed = false;
                            self.send(AppEvent::MouseUp(x, y));
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta_y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(y) * 40.0,
                    MouseScrollDelta::PixelDelta(pos) => pos.y,
                };
                self.send(AppEvent::ScrollWheel(delta_y));
            }
            WindowEvent::DroppedFile(path) => {
                self.send(AppEvent::FileDropped(vec![path
                    .to_string_lossy()
                    .to_string()]));
            }
            _ => {}
        }
    }
}

pub fn run(
    window_size: (f64, f64),
    window_position: Option<(f64, f64)>,
    setup: impl FnOnce(Arc<MacWindow>, mpsc::Receiver<AppEvent>) + 'static,
) {
    let event_loop = EventLoop::new().expect("create linux event loop");
    let mut app = GrowtermApp {
        setup: Some(setup),
        window_size,
        window_position,
        window: None,
        sender: None,
        modifiers: ModifiersState::empty(),
        cursor_position: (0.0, 0.0),
        mouse_left_pressed: false,
        ime_composing: false,
    };
    event_loop.run_app(&mut app).expect("run linux event loop");
}

/// What this crate does with a modified key press.
#[derive(Debug)]
enum Shortcut {
    /// Stand in these modifiers, so the handlers growterm-app shares with macOS run.
    AsSuper(Modifiers),
}

/// GNOME and X11 claim the Super key, so Linux app shortcuts are built on
/// Ctrl+Shift (macOS Cmd) and Alt (tab movement) instead.
fn shortcut(
    keycode: Option<u16>,
    modifiers: Modifiers,
    has_selection: bool,
) -> Option<Shortcut> {
    use crate::key_convert::keycode as kc;

    let key = keycode?;
    let ctrl = modifiers.contains(Modifiers::CONTROL);
    let shift = modifiers.contains(Modifiers::SHIFT);
    let alt = modifiers.contains(Modifiers::ALT);

    if ctrl && shift {
        // The toggles and the config reload are not here: the toggles live in
        // the dock's right-click menu, the config reloads itself when the file
        // changes, and the app running inside wants their letters.
        match key {
            // New window/tab, close tab, copy, paste, copy input line, search,
            // and scrollback — all keyed on Cmd over on macOS.
            kc::ANSI_N
            | kc::ANSI_T
            | kc::ANSI_W
            | kc::ANSI_C
            | kc::ANSI_V
            | kc::ANSI_A
            | kc::ANSI_F
            | kc::PAGE_UP
            | kc::PAGE_DOWN
            | kc::HOME
            | kc::END => return Some(Shortcut::AsSuper(Modifiers::SUPER)),
            _ => return None,
        }
    }

    if ctrl {
        // Ctrl+= / Ctrl+- : zoom
        if matches!(key, kc::ANSI_EQUAL | kc::ANSI_MINUS) {
            return Some(Shortcut::AsSuper(Modifiers::SUPER));
        }
        // Ctrl+V paste, Ctrl+A copy the input line. Both shadow a readline key,
        // which is the tradeoff asked for.
        if matches!(key, kc::ANSI_V | kc::ANSI_A) {
            return Some(Shortcut::AsSuper(Modifiers::SUPER));
        }
        // Ctrl+C copies a selection, and stays SIGINT when there is none.
        if key == kc::ANSI_C && has_selection {
            return Some(Shortcut::AsSuper(Modifiers::SUPER));
        }
    }

    if alt {
        // Alt+1~9 : switch tab by number
        let is_digit = matches!(
            key,
            kc::ANSI_1
                | kc::ANSI_2
                | kc::ANSI_3
                | kc::ANSI_4
                | kc::ANSI_5
                | kc::ANSI_6
                | kc::ANSI_7
                | kc::ANSI_8
                | kc::ANSI_9
        );
        if is_digit {
            return Some(Shortcut::AsSuper(Modifiers::SUPER));
        }
        // Alt+[ / Alt+] : previous / next tab, which growterm-app reads on SHIFT
        if matches!(key, kc::ANSI_LEFT_BRACKET | kc::ANSI_RIGHT_BRACKET) {
            return Some(Shortcut::AsSuper(Modifiers::SUPER | Modifiers::SHIFT));
        }
    }

    None
}

/// growterm-app opens the URL under a Cmd+click. GNOME owns the Super key, so
/// on Linux it is Ctrl that stands in — the same swap the keyboard already
/// makes, and what other terminals here do.
fn as_mouse_modifiers(modifiers: Modifiers) -> Modifiers {
    if modifiers.contains(Modifiers::CONTROL) {
        (modifiers - Modifiers::CONTROL) | Modifiers::SUPER
    } else {
        modifiers
    }
}

fn convert_modifiers(modifiers: ModifiersState) -> Modifiers {
    let mut out = Modifiers::empty();
    if modifiers.shift_key() {
        out |= Modifiers::SHIFT;
    }
    if modifiers.control_key() {
        out |= Modifiers::CONTROL;
    }
    if modifiers.alt_key() {
        out |= Modifiers::ALT;
    }
    if modifiers.super_key() {
        out |= Modifiers::SUPER;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key_convert::keycode as kc;

    const CTRL_SHIFT: Modifiers = Modifiers::CONTROL.union(Modifiers::SHIFT);

    fn remapped(keycode: u16, modifiers: Modifiers) -> Modifiers {
        match shortcut(Some(keycode), modifiers, false) {
            Some(Shortcut::AsSuper(m)) => m,
            other => panic!("expected a remap, got {other:?}"),
        }
    }

    #[test]
    fn ctrl_shift_c_and_v_reach_copy_and_paste() {
        assert_eq!(remapped(kc::ANSI_C, CTRL_SHIFT), Modifiers::SUPER);
        assert_eq!(remapped(kc::ANSI_V, CTRL_SHIFT), Modifiers::SUPER);
    }

    #[test]
    fn ctrl_shift_a_copies_the_input_line() {
        assert_eq!(remapped(kc::ANSI_A, CTRL_SHIFT), Modifiers::SUPER);
    }

    #[test]
    fn ctrl_shift_f_opens_search() {
        assert_eq!(remapped(kc::ANSI_F, CTRL_SHIFT), Modifiers::SUPER);
    }

    #[test]
    fn ctrl_shift_scroll_keys_reach_the_scrollback() {
        for key in [kc::PAGE_UP, kc::PAGE_DOWN, kc::HOME, kc::END] {
            assert_eq!(remapped(key, CTRL_SHIFT), Modifiers::SUPER);
        }
    }

    #[test]
    fn ctrl_shift_n_t_w_open_and_close() {
        for key in [kc::ANSI_N, kc::ANSI_T, kc::ANSI_W] {
            assert_eq!(remapped(key, CTRL_SHIFT), Modifiers::SUPER);
        }
    }

    #[test]
    fn ctrl_zoom_keys_remap_to_super() {
        for key in [kc::ANSI_EQUAL, kc::ANSI_MINUS] {
            assert_eq!(remapped(key, Modifiers::CONTROL), Modifiers::SUPER);
        }
    }

    #[test]
    fn alt_digits_switch_tabs() {
        for key in [kc::ANSI_1, kc::ANSI_5, kc::ANSI_9] {
            assert_eq!(remapped(key, Modifiers::ALT), Modifiers::SUPER);
        }
    }

    /// growterm-app cycles tabs on SUPER+SHIFT, so the shift has to survive.
    #[test]
    fn alt_brackets_cycle_tabs_with_shift_intact() {
        for key in [kc::ANSI_LEFT_BRACKET, kc::ANSI_RIGHT_BRACKET] {
            assert_eq!(
                remapped(key, Modifiers::ALT),
                Modifiers::SUPER | Modifiers::SHIFT
            );
        }
    }

    #[test]
    fn ctrl_click_stands_in_for_cmd_click() {
        assert_eq!(
            as_mouse_modifiers(Modifiers::CONTROL),
            Modifiers::SUPER,
            "GNOME keeps the Super key, so Ctrl is what a Linux user can press"
        );
    }

    #[test]
    fn ctrl_click_keeps_the_other_modifiers() {
        assert_eq!(
            as_mouse_modifiers(Modifiers::CONTROL | Modifiers::SHIFT),
            Modifiers::SUPER | Modifiers::SHIFT
        );
    }

    #[test]
    fn a_plain_click_stays_plain() {
        assert_eq!(as_mouse_modifiers(Modifiers::empty()), Modifiers::empty());
        assert_eq!(as_mouse_modifiers(Modifiers::ALT), Modifiers::ALT);
    }

    #[test]
    fn the_toggles_leave_their_letters_to_the_shell() {
        // The toggles are in the dock's right-click menu and the config
        // reloads itself, so the app running inside gets these chords —
        // Claude Code reads Ctrl+Shift+R.
        for key in [kc::ANSI_P, kc::ANSI_R, kc::ANSI_K, kc::ANSI_O, kc::ANSI_L] {
            assert!(
                shortcut(Some(key), CTRL_SHIFT, false).is_none(),
                "ctrl+shift+{key:#x} should reach the shell"
            );
        }
    }

    #[test]
    fn ctrl_v_pastes_and_ctrl_a_copies_the_input_line() {
        assert_eq!(remapped(kc::ANSI_V, Modifiers::CONTROL), Modifiers::SUPER);
        assert_eq!(remapped(kc::ANSI_A, Modifiers::CONTROL), Modifiers::SUPER);
    }

    #[test]
    fn ctrl_c_copies_a_selection() {
        assert!(matches!(
            shortcut(Some(kc::ANSI_C), Modifiers::CONTROL, true),
            Some(Shortcut::AsSuper(Modifiers::SUPER))
        ));
    }

    /// Without a selection there is nothing to copy, so the shell keeps its
    /// interrupt.
    #[test]
    fn ctrl_c_stays_sigint_with_nothing_selected() {
        assert!(shortcut(Some(kc::ANSI_C), Modifiers::CONTROL, false).is_none());
    }

    #[test]
    fn other_ctrl_keys_reach_the_terminal() {
        assert!(shortcut(Some(kc::ANSI_P), Modifiers::CONTROL, true).is_none());
        assert!(shortcut(Some(kc::ANSI_D), Modifiers::CONTROL, true).is_none());
    }

    #[test]
    fn unmodified_keys_reach_the_terminal() {
        assert!(shortcut(Some(kc::ANSI_C), Modifiers::empty(), true).is_none());
        assert!(shortcut(None, CTRL_SHIFT, false).is_none());
    }
}
