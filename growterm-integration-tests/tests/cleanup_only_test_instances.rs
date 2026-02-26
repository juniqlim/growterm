use std::process::{Command, Stdio};
use std::time::Duration;

use growterm_integration_tests::cleanup;

#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(unix)]
fn first_child_pid(parent_pid: u32) -> Option<u32> {
    let output = Command::new("pgrep")
        .args(["-P", &parent_pid.to_string()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find_map(|line| line.trim().parse::<u32>().ok())
}

#[cfg(unix)]
#[test]
fn cleanup_kills_only_spawned_process_tree() {
    let mut unrelated = Command::new("sleep")
        .arg("60")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn unrelated process");

    let mut target = Command::new("sh")
        .arg("-c")
        .arg("nohup sleep 60 >/dev/null 2>&1 & sleep 60")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn target process");

    std::thread::sleep(Duration::from_millis(200));

    let target_pid = target.id();
    let descendant_pid = first_child_pid(target_pid).expect("target child process not found");
    let unrelated_pid = unrelated.id();
    assert!(pid_alive(target_pid), "target process should be alive before cleanup");
    assert!(
        pid_alive(descendant_pid),
        "target descendant process should be alive before cleanup"
    );
    assert!(
        pid_alive(unrelated_pid),
        "unrelated process should be alive before cleanup"
    );

    let dump_path = std::env::temp_dir().join(format!(
        "growterm_cleanup_only_test_instances_{}.txt",
        std::process::id()
    ));
    let _ = std::fs::write(&dump_path, "dummy");
    cleanup(&mut target, &dump_path);

    std::thread::sleep(Duration::from_millis(200));

    assert!(!pid_alive(target_pid), "target process should be terminated");
    assert!(
        !pid_alive(descendant_pid),
        "target descendant process should be terminated"
    );
    assert!(
        pid_alive(unrelated_pid),
        "unrelated process should remain alive"
    );
    assert!(
        !dump_path.exists(),
        "cleanup should remove the dump file"
    );

    let _ = unrelated.kill();
    let _ = unrelated.wait();
}
