//! Integration test: 메인 메뉴에 Cmd+Q Quit 항목이 존재하는지 검증.
//!
//! macOS에서 Cmd+Q는 메인 메뉴의 key equivalent로 처리됨.
//! 메뉴가 올바르게 설정되어 있으면 Cmd+Q 종료가 동작함.

use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

fn main() {
    let mtm = MainThreadMarker::new().expect("must run on main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Prohibited);

    // run()을 호출하지 않고 setup_main_menu만 테스트할 수 없으므로
    // lib의 공개 API를 통해 메뉴 설정을 검증.
    // setup_main_menu는 run() 내부에서 호출되므로, 여기서는 직접 호출.
    growterm_macos::test_support::setup_menu(&app);

    let menu = app.mainMenu().expect("main menu should be set");
    let first_item = menu.itemAtIndex(0).expect("should have app menu item");
    let submenu = first_item.submenu().expect("app menu item should have submenu");

    // submenu에서 key equivalent "q"인 항목 찾기
    let mut found_quit = false;
    let count = submenu.numberOfItems();
    for i in 0..count {
        if let Some(item) = submenu.itemAtIndex(i) {
            if item.keyEquivalent().to_string() == "q" {
                found_quit = true;
                break;
            }
        }
    }
    assert!(found_quit, "main menu should have Quit item with Cmd+Q key equivalent");
    println!("PASS: main menu has Quit item with Cmd+Q");
}
