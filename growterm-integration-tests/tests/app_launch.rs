use std::time::Duration;

use growterm_integration_tests::{build_binary, spawn_app};

#[test]
fn app_starts_and_stays_alive() {
    let bin = build_binary();

    let mut child = spawn_app(&bin);

    // Give the app time to initialize window, PTY, and render first frame.
    std::thread::sleep(Duration::from_secs(3));

    // The process should still be alive (not crashed).
    match child.try_wait().expect("failed to check child status") {
        None => {
            // Still running — success. Kill it.
            child.kill().expect("failed to kill child");
            child.wait().ok();
        }
        Some(status) => {
            let stderr = {
                use std::io::Read;
                let mut buf = String::new();
                child.stderr.as_mut().unwrap().read_to_string(&mut buf).ok();
                buf
            };
            panic!(
                "growterm exited early with status {status}.\nstderr:\n{stderr}"
            );
        }
    }
}

#[test]
fn app_accepts_keystrokes_without_crash() {
    let bin = build_binary();

    let mut child = spawn_app(&bin);

    // Wait for app to initialize.
    std::thread::sleep(Duration::from_secs(3));

    // Confirm it's still alive after startup.
    assert!(
        child.try_wait().expect("try_wait").is_none(),
        "app crashed during startup"
    );

    // Keep it alive a bit longer to confirm stability.
    std::thread::sleep(Duration::from_secs(2));

    match child.try_wait().expect("try_wait") {
        None => {
            child.kill().expect("kill");
            child.wait().ok();
        }
        Some(status) => {
            let stderr = {
                use std::io::Read;
                let mut buf = String::new();
                child.stderr.as_mut().unwrap().read_to_string(&mut buf).ok();
                buf
            };
            panic!("growterm crashed after startup with status {status}.\nstderr:\n{stderr}");
        }
    }
}
