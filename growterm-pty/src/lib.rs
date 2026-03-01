use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// PTY read end. Moved to IO thread in Phase 7.
pub struct PtyReader {
    inner: Box<dyn io::Read + Send>,
}

impl io::Read for PtyReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

/// PTY write end + resize control. Stays on main thread.
pub struct PtyWriter {
    writer: Arc<Mutex<Box<dyn io::Write + Send>>>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl io::Write for PtyWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "pty writer lock poisoned"))?;
        writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "pty writer lock poisoned"))?;
        writer.flush()
    }
}

#[derive(Clone)]
pub struct PtyResponder {
    writer: Arc<Mutex<Box<dyn io::Write + Send>>>,
}

impl PtyResponder {
    pub fn write_all_flush(&self, bytes: &[u8]) -> io::Result<()> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "pty writer lock poisoned"))?;
        writer.write_all(bytes)?;
        writer.flush()
    }
}

impl PtyWriter {
    pub fn resize(&self, rows: u16, cols: u16) -> io::Result<()> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    pub fn child_pid(&self) -> Option<u32> {
        self._child.process_id()
    }

    pub fn responder(&self) -> PtyResponder {
        PtyResponder {
            writer: Arc::clone(&self.writer),
        }
    }
}

/// Spawn a shell process in a PTY.
/// Returns (reader, writer) for use on separate threads.
pub fn spawn(rows: u16, cols: u16) -> io::Result<(PtyReader, PtyWriter)> {
    spawn_with_cwd(rows, cols, None)
}

/// Spawn a shell process in a PTY with an optional working directory.
/// If `cwd` is `None`, defaults to HOME.
pub fn spawn_with_cwd(
    rows: u16,
    cols: u16,
    cwd: Option<&std::path::Path>,
) -> io::Result<(PtyReader, PtyWriter)> {
    let pty_system = NativePtySystem::default();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let mut cmd = build_shell_command(&shell);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    // .app 번들로 실행 시 launchd 환경에는 LANG이 없어 한글이 깨짐.
    // 터미널 환경에 이미 있으면 그대로 쓰고, 없으면 UTF-8로 설정.
    if std::env::var("LANG").unwrap_or_default().is_empty() {
        cmd.env("LANG", "en_US.UTF-8");
    }
    if let Some(dir) = cwd {
        cmd.cwd(dir);
    } else if let Some(home) = std::env::var_os("HOME") {
        // .app 번들에서 실행 시 cwd가 / 등이 되어 쉘 시작 스크립트가
        // 보호된 폴더에 접근하면 TCC 다이얼로그가 반복 발생함.
        cmd.cwd(home);
    }

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let writer = pair
        .master
        .take_writer()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let shared_writer = Arc::new(Mutex::new(writer));

    Ok((
        PtyReader { inner: reader },
        PtyWriter {
            writer: shared_writer,
            master: pair.master,
            _child: child,
        },
    ))
}

/// Get the current working directory of a process by PID (macOS only).
pub fn child_cwd(pid: u32) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        use std::mem;

        const PROC_PIDVNODEPATHINFO: libc::c_int = 9;

        #[repr(C)]
        struct VnodeInfoPath {
            _vip_vi: [u8; 152], // struct vnode_info (we don't need it)
            vip_path: [libc::c_char; libc::PATH_MAX as usize],
        }

        #[repr(C)]
        struct ProcVnodePathInfo {
            pvi_cdir: VnodeInfoPath,
            _pvi_rdir: VnodeInfoPath,
        }

        unsafe {
            let mut info: ProcVnodePathInfo = mem::zeroed();
            let size = mem::size_of::<ProcVnodePathInfo>() as libc::c_int;
            let ret = libc::proc_pidinfo(
                pid as libc::c_int,
                PROC_PIDVNODEPATHINFO,
                0,
                &mut info as *mut _ as *mut libc::c_void,
                size,
            );
            if ret <= 0 {
                return None;
            }
            let c_str = std::ffi::CStr::from_ptr(info.pvi_cdir.vip_path.as_ptr());
            let path = PathBuf::from(c_str.to_string_lossy().into_owned());
            if path.is_dir() {
                Some(path)
            } else {
                None
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pid;
        None
    }
}

fn build_shell_command(shell: &str) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(shell);
    // Start an interactive login shell so zprofile/login PATH setup is applied.
    cmd.arg("-l");
    cmd
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    #[test]
    fn child_cwd_returns_cwd_of_spawned_shell() {
        let (_reader, writer) = super::spawn(24, 80).unwrap();
        let pid = writer.child_pid().expect("should have child PID");
        // The spawned shell starts in HOME
        let cwd = super::child_cwd(pid);
        assert!(cwd.is_some(), "should resolve CWD for child process");
        assert!(cwd.unwrap().is_dir());
    }

    #[test]
    fn child_cwd_returns_none_for_invalid_pid() {
        assert!(super::child_cwd(0).is_none());
    }

    #[test]
    fn shell_command_includes_login_flag() {
        let cmd = super::build_shell_command("/bin/zsh");
        let argv = cmd.get_argv();
        assert!(
            argv.iter().any(|arg| arg == OsStr::new("-l")),
            "expected shell command argv to include '-l', got: {argv:?}"
        );
    }
}
