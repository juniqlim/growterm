use std::process::{Command, Stdio};
use std::time::Duration;

/// Build the juniqterm binary and return the path.
fn build_binary() -> String {
    let output = Command::new("cargo")
        .args(["build", "--package", "juniqterm-app"])
        .output()
        .expect("failed to run cargo build");
    assert!(
        output.status.success(),
        "cargo build failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let metadata = Command::new("cargo")
        .args(["metadata", "--format-version=1", "--no-deps"])
        .output()
        .expect("failed to run cargo metadata");
    let meta: serde_json::Value =
        serde_json::from_slice(&metadata.stdout).expect("invalid cargo metadata json");
    let target_dir = meta["target_directory"].as_str().unwrap();
    format!("{target_dir}/debug/juniqterm")
}

#[test]
fn app_starts_and_stays_alive() {
    let bin = build_binary();

    let mut child = Command::new(&bin)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to launch juniqterm");

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
                "juniqterm exited early with status {status}.\nstderr:\n{stderr}"
            );
        }
    }
}

#[test]
fn app_accepts_keystrokes_without_crash() {
    let bin = build_binary();

    let mut child = Command::new(&bin)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to launch juniqterm");

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
            panic!("juniqterm crashed after startup with status {status}.\nstderr:\n{stderr}");
        }
    }
}
