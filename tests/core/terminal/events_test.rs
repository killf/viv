use viv::core::terminal::events::*;
use viv::core::terminal::input::KeyEvent;
use viv::core::terminal::size::TermSize;

// FFI declarations for the fd-isolation tests below.
unsafe extern "C" {
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    fn pipe(pipefd: *mut i32) -> i32;
    fn dup(oldfd: i32) -> i32;
    fn open(path: *const u8, flags: i32, ...) -> i32;
    fn close(fd: i32) -> i32;
}

const F_GETFL: i32 = 3;
const F_SETFL: i32 = 4;
const O_NONBLOCK: i32 = 0o4000;
const O_RDWR: i32 = 2;

#[test]
fn event_loop_creation() {
    let el = EventLoop::new();
    assert!(el.is_ok());
    // Drop restores stdin flags
}

#[test]
fn poll_timeout_returns_tick() {
    let mut el = EventLoop::new().unwrap();
    let events = el.poll(1).unwrap(); // 1ms timeout
    // Either empty or Tick (no stdin data, no signal)
    for e in &events {
        assert!(matches!(e, Event::Tick));
    }
}

#[test]
fn event_debug_and_eq() {
    let a = Event::Tick;
    let b = Event::Tick;
    assert_eq!(a, b);
    let _ = format!("{:?}", Event::Resize(TermSize { cols: 80, rows: 24 }));
    let _ = format!("{:?}", Event::Key(KeyEvent::Enter));
}

/// Documents WHY EventLoop must not call `fcntl(0, F_SETFL, O_NONBLOCK)`:
/// in a shell, stdin/stdout/stderr are typically `dup`-ed from the same TTY
/// handle, so they share one "open file description". O_NONBLOCK lives on the
/// file description, not the fd — setting it on one leaks to all siblings,
/// and stdout.write_all() then fails with EAGAIN.
#[test]
fn dup_shares_nonblock_via_file_description() {
    let mut fds = [0i32; 2];
    assert_eq!(unsafe { pipe(fds.as_mut_ptr()) }, 0);
    let (r, w) = (fds[0], fds[1]);
    let r_sibling = unsafe { dup(r) };
    assert!(r_sibling >= 0);

    // Both start blocking.
    assert_eq!(unsafe { fcntl(r, F_GETFL) } & O_NONBLOCK, 0);
    assert_eq!(unsafe { fcntl(r_sibling, F_GETFL) } & O_NONBLOCK, 0);

    // Set O_NONBLOCK on r only.
    let flags = unsafe { fcntl(r, F_GETFL) };
    assert_eq!(unsafe { fcntl(r, F_SETFL, flags | O_NONBLOCK) }, 0);

    // Sibling is now non-blocking too (shared file description).
    assert_ne!(
        unsafe { fcntl(r_sibling, F_GETFL) } & O_NONBLOCK,
        0,
        "dup'd fd must share O_NONBLOCK via shared file description"
    );

    unsafe {
        close(r);
        close(r_sibling);
        close(w);
    }
}

/// Documents the fix rationale: `open()` creates a FRESH file description,
/// so O_NONBLOCK set on the new fd stays isolated. That's why EventLoop
/// opens `/dev/tty` instead of fiddling with fd 0's flags.
#[test]
fn open_creates_independent_file_description() {
    let path = b"/dev/null\0";
    let fd1 = unsafe { open(path.as_ptr(), O_RDWR) };
    let fd2 = unsafe { open(path.as_ptr(), O_RDWR) };
    assert!(fd1 >= 0 && fd2 >= 0);

    let flags = unsafe { fcntl(fd1, F_GETFL) };
    assert_eq!(unsafe { fcntl(fd1, F_SETFL, flags | O_NONBLOCK) }, 0);

    assert_eq!(
        unsafe { fcntl(fd2, F_GETFL) } & O_NONBLOCK,
        0,
        "separately-opened fd must have its own file description"
    );

    unsafe {
        close(fd1);
        close(fd2);
    }
}

/// Regression: `EventLoop::new()` must NOT flip stdin's O_NONBLOCK bit. When
/// stdin/stdout share a file description (typical shell), touching stdin's
/// flags leaks to stdout and breaks `stdout.write_all` with EAGAIN.
///
/// Holds in both envs:
///   - TTY:     EventLoop opens `/dev/tty`, never touches fd 0.
///   - non-TTY: EventLoop falls back to fd 0 but MUST NOT set O_NONBLOCK on it
///              (uses a single read-per-poll strategy instead of drain-to-EAGAIN).
#[test]
fn event_loop_does_not_modify_stdin_flags() {
    let before = unsafe { fcntl(0, F_GETFL) };
    assert!(before >= 0);

    {
        let _el = EventLoop::new().unwrap();
        let during = unsafe { fcntl(0, F_GETFL) };
        assert_eq!(
            during & O_NONBLOCK,
            before & O_NONBLOCK,
            "EventLoop::new must not change stdin's O_NONBLOCK bit \
             (would leak to stdout via shared file description)"
        );
    }

    let after = unsafe { fcntl(0, F_GETFL) };
    assert_eq!(after, before, "stdin flags must be intact after Drop");
}
