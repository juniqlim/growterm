mod delegate;
pub mod event;
pub mod key_convert;
mod view;
mod window;

pub use event::{AppEvent, Modifiers};
pub use key_convert::convert_key;
pub use window::MacWindow;

use std::sync::mpsc;

use objc2::runtime::ProtocolObject;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate};

use delegate::AppDelegate;

/// macOS 네이티브 이벤트 루프 실행.
///
/// 이 함수는 메인 스레드에서 호출되어야 하며 반환하지 않음.
/// `setup` 콜백에서 MacWindow와 이벤트 수신 채널을 받아 초기화 수행.
pub fn run(setup: impl FnOnce(std::sync::Arc<MacWindow>, mpsc::Receiver<AppEvent>) + 'static) -> ! {
    let mtm = MainThreadMarker::new().expect("must be called from main thread");

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let delegate = AppDelegate::new(mtm);
    let delegate_proto: &ProtocolObject<dyn NSApplicationDelegate> =
        ProtocolObject::from_ref(&*delegate);
    app.setDelegate(Some(delegate_proto));

    let mac_window = MacWindow::new(mtm, "juniqterm", 800.0, 600.0);
    let (tx, rx) = mpsc::channel();
    mac_window.set_sender(tx);
    mac_window.show();

    let mac_window = std::sync::Arc::new(mac_window);
    setup(mac_window, rx);

    app.run();

    std::process::exit(0);
}
