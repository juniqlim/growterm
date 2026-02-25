mod app;
#[allow(dead_code)]
mod event_action;
#[allow(dead_code)]
mod selection;
mod zoom;

const FONT_SIZE: f32 = 32.0;

fn main() {
    juniqterm_macos::run(|window, rx| {
        // GpuDrawer must be created on the main thread (Metal requirement)
        let (width, height) = window.inner_size();
        let drawer = juniqterm_gpu_draw::GpuDrawer::new(window.clone(), width, height, FONT_SIZE);

        std::thread::spawn(move || {
            app::run(window, rx, drawer);
        });
    });
}
