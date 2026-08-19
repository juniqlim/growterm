use crate::event::AppEvent;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::{Duration, SystemTime};

/// How often the config file is looked at. It changes when a person changes it,
/// so a second late is not late.
const INTERVAL: Duration = Duration::from_secs(1);

/// Remembers when the config file last changed, so a poll can tell.
pub struct ConfigWatch {
    path: PathBuf,
    seen: Option<SystemTime>,
}

impl ConfigWatch {
    pub fn new(path: PathBuf) -> Self {
        let seen = modified(&path);
        Self { path, seen }
    }

    /// True the first time a poll sees a different file than the last one.
    pub fn changed(&mut self) -> bool {
        let now = modified(&self.path);
        if now == self.seen {
            return false;
        }
        self.seen = now;
        true
    }
}

fn modified(path: &PathBuf) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// Watch the config file and tell the app whenever it changes, so editing it —
/// by hand or from the desktop's menu — is all it takes.
pub fn spawn(path: PathBuf, sender: Sender<AppEvent>) {
    std::thread::spawn(move || {
        let mut watch = ConfigWatch::new(path);
        loop {
            std::thread::sleep(INTERVAL);
            if watch.changed() && sender.send(AppEvent::ReloadConfig).is_err() {
                return;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("growterm-config-watch-{name}"));
        let _ = std::fs::remove_file(&path);
        path
    }

    fn write(path: &PathBuf, text: &str) {
        std::fs::write(path, text).unwrap();
        // Some filesystems only keep whole seconds, so make the change visible.
        let later = SystemTime::now() + Duration::from_secs(2);
        let _ = filetime(path, later);
    }

    fn filetime(path: &PathBuf, time: SystemTime) -> std::io::Result<()> {
        let file = std::fs::OpenOptions::new().write(true).open(path)?;
        file.set_modified(time)
    }

    #[test]
    fn a_file_left_alone_has_not_changed() {
        let path = temp_path("untouched");
        write(&path, "unfocused_tint = 0.1\n");
        let mut watch = ConfigWatch::new(path.clone());

        assert!(!watch.changed());
        assert!(!watch.changed());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn an_edited_file_is_reported_once() {
        let path = temp_path("edited");
        write(&path, "unfocused_tint = 0.1\n");
        let mut watch = ConfigWatch::new(path.clone());

        write(&path, "unfocused_tint = 0.5\n");

        assert!(watch.changed(), "the edit should be seen");
        assert!(!watch.changed(), "and only once");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_file_that_appears_later_counts_as_a_change() {
        let path = temp_path("appearing");
        let mut watch = ConfigWatch::new(path.clone());

        write(&path, "unfocused_tint = 0.1\n");

        assert!(watch.changed());

        let _ = std::fs::remove_file(&path);
    }
}
