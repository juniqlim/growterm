mod app;
mod ink_workaround;
#[allow(dead_code)]
mod selection;
mod tab;
mod zoom;

const FONT_SIZE: f32 = 32.0;

fn main() {
    growterm_macos::run(|window, rx| {
        // GpuDrawer must be created on the main thread (Metal requirement)
        let (width, height) = window.inner_size();
        let drawer = growterm_gpu_draw::GpuDrawer::new(window.clone(), width, height, FONT_SIZE);

        std::thread::spawn(move || {
            app::run(window, rx, drawer);
        });
    });
}
