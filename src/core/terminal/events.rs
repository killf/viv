use super::input::{InputParser, KeyEvent};
use super::size::TermSize;
use crate::core::platform::{PlatformReactor, PlatformResizeListener, PlatformTerminal};

pub const TOKEN_STDIN: u64 = 0;
pub const TOKEN_SIGNAL: u64 = 1;

/// A unified terminal event.
#[derive(Debug, PartialEq)]
pub enum Event {
    Key(KeyEvent),
    Resize(TermSize),
    Tick,
}

/// Multiplexes keyboard input and SIGWINCH resize signals via the platform reactor.
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
    reactor: PlatformReactor,
    input: InputParser,
    terminal: PlatformTerminal,
    resize: PlatformResizeListener,
    /// Whether input_fd was successfully registered with the reactor.
    stdin_in_epoll: bool,
}

impl EventLoop {
    pub fn new() -> crate::Result<Self> {
        // 1. Create platform reactor (owns its own epoll fd, separate from global)
        let reactor = PlatformReactor::new()?;

        // 2. Create PlatformResizeListener (installs SIGWINCH handler)
        let resize = PlatformResizeListener::new()?;

        // 3. Create PlatformTerminal (opens /dev/tty or falls back to fd 0)
        let terminal = PlatformTerminal::new()?;

        // 4. Register input and signal fds with the reactor
        let input_fd = terminal.input_handle();
        let stdin_in_epoll = reactor.register_fd(input_fd, TOKEN_STDIN)?;
        reactor.register_fd(resize.handle(), TOKEN_SIGNAL)?;

        // 5. Create InputParser
        let input = InputParser::new();

        Ok(EventLoop {
            reactor,
            input,
            terminal,
            resize,
            stdin_in_epoll,
        })
    }

    /// Poll for events, waiting up to `timeout_ms` milliseconds.
    /// Returns a (possibly empty) list of events. On timeout with no activity,
    /// returns a single `Event::Tick`.
    pub fn poll(&mut self, timeout_ms: i32) -> crate::Result<Vec<Event>> {
        let tokens = self.wait_events(timeout_ms)?;
        let mut events: Vec<Event> = Vec::new();

        for token in &tokens {
            match *token {
                TOKEN_STDIN => {
                    self.drain_stdin(&mut events)?;
                }
                TOKEN_SIGNAL => {
                    self.resize.drain();
                    let (rows, cols) = self.terminal.size()?;
                    events.push(Event::Resize(TermSize { rows, cols }));
                }
                _ => {}
            }
        }

        // If timed out (no tokens), also try to read stdin when it wasn't
        // registered (e.g. test environment), then emit Tick.
        if tokens.is_empty() {
            if !self.stdin_in_epoll {
                self.drain_stdin(&mut events)?;
            }
            events.push(Event::Tick);
        }

        Ok(events)
    }

    /// Wait for reactor events, return list of fired tokens.
    fn wait_events(&self, timeout_ms: i32) -> crate::Result<Vec<u64>> {
        self.reactor.wait_tokens(timeout_ms)
    }

    fn drain_stdin(&mut self, events: &mut Vec<Event>) -> crate::Result<()> {
        let mut buf = [0u8; 4096];
        if self.terminal.owns_input() {
            // Owned fd is non-blocking — safe to drain until EAGAIN/0.
            loop {
                match self.terminal.read_input(&mut buf) {
                    Ok(n) => {
                        if n > 0 {
                            self.input.feed(&buf[..n]);
                        }
                        if n == 0 || n < buf.len() {
                            break; // EOF or flushed buffer
                        }
                    }
                    Err(crate::Error::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        break; // no more input right now
                    }
                    Err(e) => return Err(e),
                }
            }
        } else {
            // Borrowed fd 0 — flags are untouchable. Epoll level-triggered woke
            // us because data is ready, so one blocking read returns immediately;
            // residual data triggers another wake on the next poll.
            let n = self.terminal.read_input(&mut buf)?;
            if n > 0 {
                self.input.feed(&buf[..n]);
            }
        }
        while let Some(key) = self.input.next_event() {
            events.push(Event::Key(key));
        }
        Ok(())
    }
}
