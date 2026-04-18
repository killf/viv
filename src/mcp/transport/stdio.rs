#[cfg(unix)]
use std::collections::HashMap;
#[cfg(unix)]
use std::future::Future;
#[cfg(unix)]
use std::pin::Pin;
#[cfg(unix)]
use std::process::{Child, Command, Stdio};
#[cfg(unix)]
use std::task::{Context, Poll};

#[cfg(unix)]
use crate::core::json::JsonValue;
#[cfg(unix)]
use crate::core::platform::RawHandle;
#[cfg(unix)]
use crate::core::runtime::reactor::reactor;

#[cfg(unix)]
use super::Transport;

// ── Unix FFI and helpers ────────────────────────────────────────────────────

#[cfg(unix)]
mod unix_io {
    use crate::core::platform::RawHandle;

    unsafe extern "C" {
        pub fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
        pub fn write(fd: i32, buf: *const u8, count: usize) -> isize;
        fn fcntl(fd: i32, cmd: i32, ...) -> i32;
        pub fn __errno_location() -> *mut i32;
    }

    const F_GETFL: i32 = 3;
    const F_SETFL: i32 = 4;
    const O_NONBLOCK: i32 = 0o4000;

    pub const EAGAIN: i32 = 11;
    pub const EWOULDBLOCK: i32 = 11; // same as EAGAIN on Linux

    /// Set a file descriptor to non-blocking mode.
    pub fn set_nonblocking(fd: RawHandle) -> crate::Result<()> {
        unsafe {
            let flags = fcntl(fd, F_GETFL);
            if flags < 0 {
                return Err(crate::Error::Io(std::io::Error::last_os_error()));
            }
            let ret = fcntl(fd, F_SETFL, flags | O_NONBLOCK);
            if ret < 0 {
                return Err(crate::Error::Io(std::io::Error::last_os_error()));
            }
        }
        Ok(())
    }

    /// Get the raw fd from a ChildStdin via the AsRawFd trait.
    pub fn stdin_fd(child: &std::process::Child) -> Option<RawHandle> {
        use std::os::unix::io::AsRawFd;
        child.stdin.as_ref().map(|s| s.as_raw_fd())
    }

    pub fn stdout_fd(child: &std::process::Child) -> Option<RawHandle> {
        use std::os::unix::io::AsRawFd;
        child.stdout.as_ref().map(|s| s.as_raw_fd())
    }
}

#[cfg(unix)]
use unix_io::*;

// ── Framing ──────────────────────────────────────────────────────────────────

/// Wire-framing strategy for `StdioTransport`.
///
/// - `Newline` — newline-delimited JSON (standard MCP over stdio).
/// - `ContentLength` — LSP-style `Content-Length: N\r\n\r\n{body}` framing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
    /// Each message is a single line terminated by `\n`.
    Newline,
    /// Each message is preceded by a `Content-Length: <n>\r\n\r\n` header.
    ContentLength,
}

impl Framing {
    /// Encode `body` into a frame ready for transmission.
    pub fn encode(&self, body: &str) -> String {
        match self {
            Framing::Newline => format!("{}\n", body),
            Framing::ContentLength => {
                format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
            }
        }
    }

    /// Try to extract one complete message from `buf`.
    ///
    /// Returns `Some(message)` and drains the consumed bytes from `buf` when a
    /// complete frame is available. Returns `None` (leaving `buf` intact) when
    /// more data is needed.
    pub fn try_decode(&self, buf: &mut Vec<u8>) -> Option<String> {
        match self {
            Framing::Newline => {
                let newline_pos = buf.iter().position(|&b| b == b'\n')?;
                let line: Vec<u8> = buf.drain(..=newline_pos).collect();
                // Trim the trailing `\n` (and any `\r` for robustness)
                let end = if line.len() >= 2 && line[line.len() - 2] == b'\r' {
                    line.len() - 2
                } else {
                    line.len() - 1
                };
                String::from_utf8(line[..end].to_vec()).ok()
            }
            Framing::ContentLength => {
                // Locate the header separator \r\n\r\n
                let sep = b"\r\n\r\n";
                let header_end = buf.windows(sep.len()).position(|w| w == sep)?;
                let header = std::str::from_utf8(&buf[..header_end]).ok()?;

                // Parse `Content-Length: <n>` from the header block
                let content_length = header
                    .lines()
                    .find_map(|line| {
                        let lower = line.to_ascii_lowercase();
                        lower.strip_prefix("content-length:").map(|v| {
                            v.trim().parse::<usize>().ok()
                        })
                    })
                    .flatten()?;

                let body_start = header_end + sep.len();

                // Not enough body bytes yet — return without mutating buf
                if buf.len() < body_start + content_length {
                    return None;
                }

                let frame_end = body_start + content_length;
                let body_bytes: Vec<u8> = buf.drain(..frame_end).collect();
                let body_str = String::from_utf8(body_bytes[body_start..].to_vec()).ok()?;
                Some(body_str)
            }
        }
    }
}

// ── WaitReadable future ──────────────────────────────────────────────────────

#[cfg(unix)]
struct WaitReadable {
    fd: RawHandle,
    token: Option<u64>,
}

#[cfg(unix)]
impl Future for WaitReadable {
    type Output = crate::Result<()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.token.is_some() {
            // We were woken — the fd is readable
            self.token = None;
            return Poll::Ready(Ok(()));
        }
        let t = reactor().lock().unwrap().register_readable(self.fd, cx.waker().clone());
        self.token = Some(t);
        Poll::Pending
    }
}

#[cfg(unix)]
impl Drop for WaitReadable {
    fn drop(&mut self) {
        if let Some(t) = self.token.take() {
            reactor().lock().unwrap().remove(t);
        }
    }
}

#[cfg(unix)]
fn wait_readable(fd: RawHandle) -> WaitReadable {
    WaitReadable { fd, token: None }
}

// ── StdioTransport ───────────────────────────────────────────────────────────

/// MCP transport over child process stdin/stdout.
///
/// Spawns a child process and communicates using the specified [`Framing`]
/// strategy over the child's stdin (send) and stdout (recv).  The child's
/// stdout is set to non-blocking and registered with the reactor for async
/// reads.
///
/// Use [`StdioTransport::spawn`] for standard newline-delimited MCP servers,
/// or [`StdioTransport::spawn_with_framing`] when a different framing is
/// required (e.g. LSP servers that use `Content-Length` headers).
#[cfg(unix)]
pub struct StdioTransport {
    child: Child,
    stdin_fd: RawHandle,
    stdout_fd: RawHandle,
    read_buf: Vec<u8>,
    framing: Framing,
}

#[cfg(unix)]
impl StdioTransport {
    /// Spawn a child process for MCP communication using [`Framing::Newline`].
    ///
    /// `command` — the executable to run (e.g. "npx", "python3")
    /// `args`    — command line arguments
    /// `env`     — additional environment variables
    pub fn spawn(
        command: &str,
        args: &[&str],
        env: &HashMap<String, String>,
    ) -> crate::Result<Self> {
        Self::spawn_with_framing(command, args, env, Framing::Newline)
    }

    /// Spawn a child process with an explicit framing strategy.
    ///
    /// Use this when connecting to an LSP server that requires
    /// [`Framing::ContentLength`] framing.
    pub fn spawn_with_framing(
        command: &str,
        args: &[&str],
        env: &HashMap<String, String>,
        framing: Framing,
    ) -> crate::Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .envs(env.iter())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = cmd.spawn().map_err(crate::Error::Io)?;

        let stdin_raw = stdin_fd(&child)
            .ok_or_else(|| crate::Error::Io(std::io::Error::other(
                "failed to get child stdin fd",
            )))?;

        let stdout_raw = stdout_fd(&child)
            .ok_or_else(|| crate::Error::Io(std::io::Error::other(
                "failed to get child stdout fd",
            )))?;

        // Set stdout to non-blocking for reactor-based async reads
        set_nonblocking(stdout_raw)?;

        Ok(StdioTransport {
            child,
            stdin_fd: stdin_raw,
            stdout_fd: stdout_raw,
            read_buf: Vec::with_capacity(4096),
            framing,
        })
    }
}

#[cfg(unix)]
impl Transport for StdioTransport {
    fn send(&mut self, msg: JsonValue) -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send + '_>> {
        Box::pin(async move {
            let data = self.framing.encode(&msg.to_string());
            let bytes = data.as_bytes();
            let mut written = 0;

            while written < bytes.len() {
                let n = unsafe {
                    write(self.stdin_fd, bytes[written..].as_ptr(), bytes[written..].len())
                };
                if n < 0 {
                    let errno = unsafe { *__errno_location() };
                    if errno == EAGAIN || errno == EWOULDBLOCK {
                        // stdin is full — unlikely for pipes but handle gracefully
                        std::thread::yield_now();
                        continue;
                    }
                    return Err(crate::Error::Io(std::io::Error::from_raw_os_error(errno)));
                }
                written += n as usize;
            }

            Ok(())
        })
    }

    fn recv(&mut self) -> Pin<Box<dyn Future<Output = crate::Result<JsonValue>> + Send + '_>> {
        Box::pin(async move {
            loop {
                // Check if we already have a complete frame in the buffer
                if let Some(msg) = self.framing.try_decode(&mut self.read_buf) {
                    if msg.is_empty() {
                        // Skip empty messages (can happen with newline framing)
                        continue;
                    }
                    return JsonValue::parse(&msg);
                }

                // No complete frame yet — wait for the fd to be readable
                wait_readable(self.stdout_fd).await?;

                // Non-blocking read
                let mut tmp = [0u8; 4096];
                let n = unsafe { read(self.stdout_fd, tmp.as_mut_ptr(), tmp.len()) };

                if n > 0 {
                    self.read_buf.extend_from_slice(&tmp[..n as usize]);
                } else if n == 0 {
                    // EOF — child closed stdout
                    return Err(crate::Error::Io(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "MCP server closed connection",
                    )));
                } else {
                    let errno = unsafe { *__errno_location() };
                    if errno == EAGAIN || errno == EWOULDBLOCK {
                        // Spurious wakeup — just loop and wait again
                        continue;
                    }
                    return Err(crate::Error::Io(std::io::Error::from_raw_os_error(errno)));
                }
            }
        })
    }

    fn close(&mut self) -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send + '_>> {
        Box::pin(async move {
            // Drop stdin to signal EOF to the child
            self.child.stdin.take();
            // Kill the child process
            self.child.kill().ok();
            // Wait for it to exit
            self.child.wait().map_err(crate::Error::Io)?;
            Ok(())
        })
    }
}
