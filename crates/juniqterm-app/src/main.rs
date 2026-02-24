mod app;
mod key_convert;

use app::App;
use winit::event_loop::EventLoop;

fn main() {
    let event_loop = EventLoop::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();
    let mut app = App::new(proxy);
    event_loop.run_app(&mut app).unwrap();
}
