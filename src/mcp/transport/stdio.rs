use std::collections::HashMap;
use std::future::Future;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::process::{Child, Command, Stdio};
use std::task::{Context, Poll};

use crate::core::json::JsonValue;
use crate::core::runtime::reactor::reactor;

use super::Transport;

// FFI for non-blocking I/O
unsafe extern "C" {
    fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    fn __errno_location() -> *mut i32;
}

const F_GETFL: i32 = 3;
const F_SETFL: i32 = 4;
const O_NONBLOCK: i32 = 0o4000;
const EAGAIN: i32 = 11;
const EWOULDBLOCK: i32 = 11; // same as EAGAIN on Linux

/// Set a file descriptor to non-blocking mode.
fn set_nonblocking(fd: RawFd) -> crate::Result<()> {
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

/// Get the raw fd from a ChildStdin/ChildStdout via the AsRawFd trait.
fn stdin_fd(child: &Child) -> Option<RawFd> {
    use std::os::unix::io::AsRawFd;
    child.stdin.as_ref().map(|s| s.as_raw_fd())
}

fn stdout_fd(child: &Child) -> Option<RawFd> {
    use std::os::unix::io::AsRawFd;
    child.stdout.as_ref().map(|s| s.as_raw_fd())
}

// ── WaitReadable future ──────────────────────────────────────────────────────

struct WaitReadable {
    fd: RawFd,
    token: Option<u64>,
}

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

impl Drop for WaitReadable {
    fn drop(&mut self) {
        if let Some(t) = self.token.take() {
            reactor().lock().unwrap().remove(t);
        }
    }
}

fn wait_readable(fd: RawFd) -> WaitReadable {
    WaitReadable { fd, token: None }
}

// ── StdioTransport ───────────────────────────────────────────────────────────

/// MCP transport over child process stdin/stdout.
///
/// Spawns a child process and communicates using newline-delimited JSON
/// over the child's stdin (send) and stdout (recv). The child's stdout
/// is set to non-blocking and registered with the reactor for async reads.
pub struct StdioTransport {
    child: Child,
    stdin_fd: RawFd,
    stdout_fd: RawFd,
    read_buf: Vec<u8>,
}

impl StdioTransport {
    /// Spawn a child process for MCP communication.
    ///
    /// `command` — the executable to run (e.g. "npx", "python3")
    /// `args` — command line arguments
    /// `env` — additional environment variables
    pub fn spawn(
        command: &str,
        args: &[&str],
        env: &HashMap<String, String>,
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
        })
    }
}

impl Transport for StdioTransport {
    fn send(&mut self, msg: JsonValue) -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send + '_>> {
        Box::pin(async move {
            // Serialize JSON + newline
            let data = format!("{}\n", msg);
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
                        // For now, yield and retry
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
                // Check if we already have a complete line in the buffer
                if let Some(newline_pos) = self.read_buf.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = self.read_buf.drain(..=newline_pos).collect();
                    // Trim the trailing newline
                    let json_str = std::str::from_utf8(&line[..line.len() - 1])
                        .map_err(|e| crate::Error::Json(format!("invalid UTF-8: {}", e)))?;

                    if json_str.is_empty() {
                        // Skip empty lines
                        continue;
                    }

                    return JsonValue::parse(json_str);
                }

                // No complete line yet — wait for the fd to be readable
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
