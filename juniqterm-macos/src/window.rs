use std::ptr::NonNull;
use std::sync::mpsc::Sender;

use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSBackingStoreType, NSWindow, NSWindowStyleMask};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle,
};

use crate::event::AppEvent;
use crate::view::TerminalView;

pub struct MacWindow {
    ns_window: Retained<NSWindow>,
    view: Retained<TerminalView>,
}

impl MacWindow {
    pub fn new(mtm: MainThreadMarker, title: &str, width: f64, height: f64) -> Self {
        let content_rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height));
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable;

        let ns_window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                mtm.alloc(),
                content_rect,
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };

        let view = TerminalView::new(mtm);

        ns_window.setContentView(Some(&view));
        ns_window.makeFirstResponder(Some(&view));

        let title_str = NSString::from_str(title);
        ns_window.setTitle(&title_str);
        ns_window.center();

        Self { ns_window, view }
    }

    pub fn set_sender(&self, sender: Sender<AppEvent>) {
        self.view.set_sender(sender);
    }

    pub fn inner_size(&self) -> (u32, u32) {
        let frame = self.view.frame();
        let scale = self.backing_scale_factor();
        let w = (frame.size.width * scale) as u32;
        let h = (frame.size.height * scale) as u32;
        (w.max(1), h.max(1))
    }

    pub fn backing_scale_factor(&self) -> f64 {
        self.ns_window.backingScaleFactor()
    }

    pub fn request_redraw(&self) {
        self.view.setNeedsDisplay(true);
    }

    pub fn show(&self) {
        self.ns_window.makeKeyAndOrderFront(None);
    }

    pub fn ns_window(&self) -> &NSWindow {
        &self.ns_window
    }
}

unsafe impl Send for MacWindow {}
unsafe impl Sync for MacWindow {}

impl HasWindowHandle for MacWindow {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        let view_ptr = Retained::as_ptr(&self.view) as *mut std::ffi::c_void;
        let non_null = NonNull::new(view_ptr).expect("view pointer should not be null");
        let handle = AppKitWindowHandle::new(non_null);
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(RawWindowHandle::AppKit(handle)) })
    }
}

impl HasDisplayHandle for MacWindow {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        let handle = AppKitDisplayHandle::new();
        Ok(unsafe {
            raw_window_handle::DisplayHandle::borrow_raw(RawDisplayHandle::AppKit(handle))
        })
    }
}
