use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::io;

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
    writer: Box<dyn io::Write + Send>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl io::Write for PtyWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
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
}

/// Spawn a shell process in a PTY.
/// Returns (reader, writer) for use on separate threads.
pub fn spawn(rows: u16, cols: u16) -> io::Result<(PtyReader, PtyWriter)> {
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
    let mut cmd = CommandBuilder::new(&shell);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    // .app 번들로 실행 시 launchd 환경에는 LANG이 없어 한글이 깨짐.
    // 터미널 환경에 이미 있으면 그대로 쓰고, 없으면 UTF-8로 설정.
    if std::env::var("LANG").unwrap_or_default().is_empty() {
        cmd.env("LANG", "en_US.UTF-8");
    }
    // .app 번들에서 실행 시 cwd가 / 등이 되어 쉘 시작 스크립트가
    // 보호된 폴더에 접근하면 TCC 다이얼로그가 반복 발생함.
    if let Some(home) = std::env::var_os("HOME") {
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

    Ok((
        PtyReader { inner: reader },
        PtyWriter {
            writer,
            master: pair.master,
            _child: child,
        },
    ))
}
