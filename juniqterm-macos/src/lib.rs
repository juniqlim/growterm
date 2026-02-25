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
/// `setup` 콜백은 applicationDidFinishLaunching에서 호출되어
/// IMK 입력 서버 연결이 완료된 상태에서 윈도우가 생성됨.
pub fn run(setup: impl FnOnce(std::sync::Arc<MacWindow>, mpsc::Receiver<AppEvent>) + 'static) -> ! {
    // bare 바이너리(cargo run)에서도 IMK 입력 서버가 연결되도록
    // 번들 ID를 런타임에 설정. .app 번들로 실행 시에는 Info.plist 값이 이미 있으므로 무해.
    ensure_bundle_identifier();

    let mtm = MainThreadMarker::new().expect("must be called from main thread");

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let delegate = AppDelegate::new(mtm, Box::new(setup));
    let delegate_proto: &ProtocolObject<dyn NSApplicationDelegate> =
        ProtocolObject::from_ref(&*delegate);
    app.setDelegate(Some(delegate_proto));

    app.run();

    std::process::exit(0);
}

/// bare 바이너리(cargo run)에서도 IMK 입력 서버가 연결되도록
/// 실행 파일을 감싸는 임시 .app 번들 구조를 만들고,
/// 실행 파일을 심볼릭 링크로 연결한다.
/// .app 번들로 실행 시에는 아무 작업도 하지 않는다.
fn ensure_bundle_identifier() {
    let Ok(exe) = std::env::current_exe() else { return };
    let exe = exe.canonicalize().unwrap_or(exe);
    let Some(dir) = exe.parent() else { return };

    // 이미 .app 번들 내부면 skip
    if dir.to_string_lossy().contains(".app/") {
        return;
    }

    let exe_name = exe.file_name().unwrap().to_string_lossy();
    let app_dir = dir.join(format!("{exe_name}.app"));
    let macos_dir = app_dir.join("Contents/MacOS");
    let plist_path = app_dir.join("Contents/Info.plist");
    let link_path = macos_dir.join(exe_name.as_ref());

    if plist_path.exists() && link_path.exists() {
        // 바이너리가 갱신되었으면 복사본도 갱신
        let src_modified = std::fs::metadata(&exe).and_then(|m| m.modified()).ok();
        let dst_modified = std::fs::metadata(&link_path).and_then(|m| m.modified()).ok();
        match (src_modified, dst_modified) {
            (Some(src), Some(dst)) if src <= dst => return,
            _ => {}
        }
    }

    let _ = std::fs::create_dir_all(&macos_dir);

    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>{exe_name}</string>
    <key>CFBundleIdentifier</key>
    <string>com.juniqlim.juniqterm</string>
    <key>CFBundleName</key>
    <string>JuniqTerm</string>
</dict>
</plist>"#
    );
    let _ = std::fs::write(&plist_path, content);

    // 바이너리를 .app 번들에 복사 (심볼릭 링크 대신)
    let _ = std::fs::remove_file(&link_path);
    let _ = std::fs::copy(&exe, &link_path);

    // `open` 명령으로 .app 번들을 실행 (Launch Services 등록 필요)
    let _ = std::process::Command::new("open")
        .arg(&app_dir)
        .arg("--args")
        .args(std::env::args_os().skip(1))
        .spawn();

    std::process::exit(0);
}
