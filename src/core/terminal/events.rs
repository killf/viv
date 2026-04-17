use super::super::event::Epoll;
use super::input::{InputParser, KeyEvent};
use super::signal::SignalPipe;
use super::size::{terminal_size, TermSize};

// FFI declarations (reuse from signal.rs pattern)
unsafe extern "C" {
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
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
const O_NONBLOCK: i32 = 0o4000;
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

/// Multiplexes stdin key events and SIGWINCH resize signals via epoll.
pub struct EventLoop {
    epoll: Epoll,
    input: InputParser,
    signal: SignalPipe,
    /// Whether stdin was successfully registered with epoll.
    stdin_in_epoll: bool,
    /// Original stdin flags, restored on Drop.
    orig_stdin_flags: i32,
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

impl EventLoop {
    pub fn new() -> crate::Result<Self> {
        // 1. Create Epoll
        let epoll = Epoll::new()?;

        // 2. Create SignalPipe (installs SIGWINCH handler)
        let signal = SignalPipe::new()?;

        // 3. Save original stdin flags
        let orig_stdin_flags = unsafe { fcntl(0, F_GETFL) };
        if orig_stdin_flags < 0 {
            return Err(crate::Error::Terminal(
                "fcntl(F_GETFL) on stdin failed".to_string(),
            ));
        }

        // 4. Set stdin non-blocking
        let ret = unsafe { fcntl(0, F_SETFL, orig_stdin_flags | O_NONBLOCK) };
        if ret < 0 {
            return Err(crate::Error::Terminal(
                "fcntl(F_SETFL) on stdin failed".to_string(),
            ));
        }

        // 5. Register stdin with epoll (best-effort: /dev/null redirects in tests
        //    return EPERM and are silently skipped)
        let epoll_fd = epoll_raw_fd(&epoll);
        let stdin_in_epoll = epoll_try_add(epoll_fd, 0, TOKEN_STDIN)?;

        // 6. Register signal pipe read end with epoll
        epoll.add(signal.read_fd(), TOKEN_SIGNAL)?;

        // 7. Create InputParser
        let input = InputParser::new();

        Ok(EventLoop {
            epoll,
            input,
            signal,
            stdin_in_epoll,
            orig_stdin_flags,
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
                    // Non-blocking drain of stdin
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
        loop {
            let n = unsafe { read(0, buf.as_mut_ptr(), buf.len()) };
            if n > 0 {
                self.input.feed(&buf[..n as usize]);
            } else if n == 0 {
                // EOF on stdin
                break;
            } else {
                let errno = unsafe { *__errno_location() };
                if errno == EAGAIN {
                    break; // no more data right now
                }
                return Err(crate::Error::Terminal(format!(
                    "read() on stdin failed: errno {}",
                    errno
                )));
            }
            // If we read less than the full buffer there's no more data immediately
            if (n as usize) < buf.len() {
                break;
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
        // Restore original stdin flags (clear O_NONBLOCK if it wasn't set before)
        unsafe {
            fcntl(0, F_SETFL, self.orig_stdin_flags);
        }
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
