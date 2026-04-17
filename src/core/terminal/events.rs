use super::super::event::Epoll;
use super::input::{InputParser, KeyEvent};
use super::signal::SignalPipe;
use super::size::{terminal_size, TermSize};

// FFI declarations (reuse from signal.rs pattern)
unsafe extern "C" {
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    fn open(path: *const u8, flags: i32, ...) -> i32;
    fn close(fd: i32) -> i32;
    fn __errno_location() -> *mut i32;
    fn epoll_ctl(epfd: i32, op: i32, fd: i32, event: *mut EpollEventRaw) -> i32;
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct EpollEventRaw {
    events: u32,
    data: u64,
}

const EPOLL_CTL_ADD: i32 = 1;
const EPOLLIN: u32 = 0x001;

const F_GETFL: i32 = 3;
const F_SETFL: i32 = 4;
const O_RDONLY: i32 = 0;
const O_NONBLOCK: i32 = 0o4000;
const O_CLOEXEC: i32 = 0o2000000;
const EAGAIN: i32 = 11;
/// EPERM: operation not permitted — e.g. epoll on /dev/null (test redirects)
const EPERM: i32 = 1;

pub const TOKEN_STDIN: u64 = 0;
pub const TOKEN_SIGNAL: u64 = 1;

/// A unified terminal event.
#[derive(Debug, PartialEq)]
pub enum Event {
    Key(KeyEvent),
    Resize(TermSize),
    Tick,
}

/// Multiplexes keyboard input and SIGWINCH resize signals via epoll.
///
/// Input fd selection (in order):
/// 1. `open("/dev/tty", O_RDONLY)` — a fresh "open file description" so we can
///    freely set `O_NONBLOCK` without leaking it to stdout/stderr (which
///    typically share fd 0's file description via `dup`). This is the happy path
///    in real terminals.
/// 2. Fall back to fd 0 when `/dev/tty` isn't available (sandboxed tests, CI).
///    In this mode we MUST NOT touch fd 0's flags — drain_stdin does a single
///    read per epoll wake instead of a drain-to-EAGAIN loop.
pub struct EventLoop {
    epoll: Epoll,
    input: InputParser,
    signal: SignalPipe,
    /// Keyboard input fd.
    input_fd: i32,
    /// True when we opened `/dev/tty` and must `close` + blindly drain.
    /// False when we fell back to fd 0 — don't touch its flags, single read per wake.
    owns_input_fd: bool,
    /// Whether input_fd was successfully registered with epoll.
    stdin_in_epoll: bool,
}

/// Try to add `fd` to `epoll_fd` with the given token.
/// Returns Ok(true) on success, Ok(false) if EPERM (fd not epoll-able),
/// Err on any other failure.
fn epoll_try_add(epoll_fd: i32, fd: i32, token: u64) -> crate::Result<bool> {
    let mut ev = EpollEventRaw {
        events: EPOLLIN,
        data: token,
    };
    let ret = unsafe { epoll_ctl(epoll_fd, EPOLL_CTL_ADD, fd, &mut ev) };
    if ret == 0 {
        return Ok(true);
    }
    let errno = unsafe { *__errno_location() };
    if errno == EPERM {
        return Ok(false); // fd not epoll-able (e.g. /dev/null in tests)
    }
    Err(crate::Error::Terminal(format!(
        "epoll_ctl add failed: errno {}",
        errno
    )))
}

/// Try to open `/dev/tty` as a fresh fd (isolated file description).
/// Returns `Some(fd)` on success or `None` if unavailable.
fn open_tty() -> Option<i32> {
    let path = b"/dev/tty\0";
    let fd = unsafe { open(path.as_ptr(), O_RDONLY | O_CLOEXEC) };
    if fd < 0 { None } else { Some(fd) }
}

impl EventLoop {
    pub fn new() -> crate::Result<Self> {
        // 1. Create Epoll
        let epoll = Epoll::new()?;

        // 2. Create SignalPipe (installs SIGWINCH handler)
        let signal = SignalPipe::new()?;

        // 3. Pick an input fd. Prefer /dev/tty (fresh file description); fall
        //    back to fd 0 when unavailable.
        let (input_fd, owns_input_fd) = match open_tty() {
            Some(fd) => (fd, true),
            None => (0, false),
        };

        // 4. Set non-blocking ONLY on owned fds. Touching fd 0's flags would
        //    leak O_NONBLOCK to stdout/stderr via the shared file description.
        if owns_input_fd {
            let flags = unsafe { fcntl(input_fd, F_GETFL) };
            if flags < 0 {
                unsafe { close(input_fd) };
                return Err(crate::Error::Terminal(
                    "fcntl(F_GETFL) on /dev/tty failed".to_string(),
                ));
            }
            let ret = unsafe { fcntl(input_fd, F_SETFL, flags | O_NONBLOCK) };
            if ret < 0 {
                unsafe { close(input_fd) };
                return Err(crate::Error::Terminal(
                    "fcntl(F_SETFL) on /dev/tty failed".to_string(),
                ));
            }
        }

        // 5. Register input fd with epoll (best-effort: /dev/null redirects in
        //    tests return EPERM and are silently skipped)
        let epoll_fd = epoll_raw_fd(&epoll);
        let stdin_in_epoll = match epoll_try_add(epoll_fd, input_fd, TOKEN_STDIN) {
            Ok(v) => v,
            Err(e) => {
                if owns_input_fd {
                    unsafe { close(input_fd) };
                }
                return Err(e);
            }
        };

        // 6. Register signal pipe read end with epoll
        epoll.add(signal.read_fd(), TOKEN_SIGNAL)?;

        // 7. Create InputParser
        let input = InputParser::new();

        Ok(EventLoop {
            epoll,
            input,
            signal,
            input_fd,
            owns_input_fd,
            stdin_in_epoll,
        })
    }

    /// Poll for events, waiting up to `timeout_ms` milliseconds.
    /// Returns a (possibly empty) list of events. On timeout with no activity,
    /// returns a single `Event::Tick`.
    pub fn poll(&mut self, timeout_ms: i32) -> crate::Result<Vec<Event>> {
        let tokens = self.epoll.wait(timeout_ms)?;
        let mut events: Vec<Event> = Vec::new();

        for token in &tokens {
            match *token {
                TOKEN_STDIN => {
                    self.drain_stdin(&mut events)?;
                }
                TOKEN_SIGNAL => {
                    self.signal.drain()?;
                    let size = terminal_size()?;
                    events.push(Event::Resize(size));
                }
                _ => {}
            }
        }

        // If epoll timed out (no tokens), also try to read stdin when it wasn't
        // registered (e.g. test environment), then emit Tick.
        if tokens.is_empty() {
            if !self.stdin_in_epoll {
                self.drain_stdin(&mut events)?;
            }
            events.push(Event::Tick);
        }

        Ok(events)
    }

    fn drain_stdin(&mut self, events: &mut Vec<Event>) -> crate::Result<()> {
        let mut buf = [0u8; 4096];
        if self.owns_input_fd {
            // Owned fd is non-blocking — safe to drain until EAGAIN.
            loop {
                let n = unsafe { read(self.input_fd, buf.as_mut_ptr(), buf.len()) };
                if n > 0 {
                    self.input.feed(&buf[..n as usize]);
                } else if n == 0 {
                    break; // EOF
                } else {
                    let errno = unsafe { *__errno_location() };
                    if errno == EAGAIN {
                        break;
                    }
                    return Err(crate::Error::Terminal(format!(
                        "read() on input fd failed: errno {}",
                        errno
                    )));
                }
                if (n as usize) < buf.len() {
                    break;
                }
            }
        } else {
            // Borrowed fd 0 — flags are untouchable. Epoll level-triggered woke
            // us because data is ready, so one blocking read returns immediately;
            // residual data triggers another wake on the next poll.
            let n = unsafe { read(self.input_fd, buf.as_mut_ptr(), buf.len()) };
            if n > 0 {
                self.input.feed(&buf[..n as usize]);
            } else if n < 0 {
                let errno = unsafe { *__errno_location() };
                if errno != EAGAIN {
                    return Err(crate::Error::Terminal(format!(
                        "read() on input fd failed: errno {}",
                        errno
                    )));
                }
            }
        }
        while let Some(key) = self.input.next_event() {
            events.push(Event::Key(key));
        }
        Ok(())
    }
}

impl Drop for EventLoop {
    fn drop(&mut self) {
        if self.owns_input_fd {
            unsafe { close(self.input_fd) };
        }
        // Borrowed fd 0: flags were never modified, so nothing to restore.
    }
}

/// Extract the raw epoll fd from an `Epoll` instance via a private accessor.
/// Since `Epoll` doesn't expose its fd, we use a workaround: call epoll_ctl
/// directly in `epoll_try_add` instead of going through `Epoll::add`.
/// This function returns the fd by reading it from the Epoll struct memory.
/// SAFETY: Epoll is repr(transparent)-ish — its only field is `fd: RawFd`.
fn epoll_raw_fd(epoll: &Epoll) -> i32 {
    // Epoll { fd: RawFd } — RawFd is i32, it's the first (only) field.
    // We read it via a pointer cast. This is sound because the layout is known.
    let ptr = epoll as *const Epoll as *const i32;
    unsafe { *ptr }
}
