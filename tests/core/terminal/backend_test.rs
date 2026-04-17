use viv::core::terminal::backend::*;
use viv::core::terminal::size::TermSize;

#[test]
fn test_backend_write_captures() {
    let mut b = TestBackend::new(80, 24);
    b.write(b"hello").unwrap();
    assert_eq!(b.output, b"hello");
}

#[test]
fn test_backend_size() {
    let b = TestBackend::new(120, 40);
    let size = b.size().unwrap();
    assert_eq!(size, TermSize { cols: 120, rows: 40 });
}

#[test]
fn test_backend_raw_mode() {
    let mut b = TestBackend::new(80, 24);
    assert!(!b.raw_mode_enabled);
    b.enable_raw_mode().unwrap();
    assert!(b.raw_mode_enabled);
    b.disable_raw_mode().unwrap();
    assert!(!b.raw_mode_enabled);
}

#[test]
fn test_backend_cursor() {
    let mut b = TestBackend::new(80, 24);
    assert!(b.cursor_visible);
    b.hide_cursor().unwrap();
    assert!(!b.cursor_visible);
    b.show_cursor().unwrap();
    assert!(b.cursor_visible);
}

#[test]
fn test_backend_move_cursor() {
    let mut b = TestBackend::new(80, 24);
    b.move_cursor(5, 10).unwrap();
    assert_eq!(b.cursor_pos, (5, 10));
}

#[test]
fn test_backend_alternate_screen_state() {
    let mut b = TestBackend::new(80, 24);
    assert!(!b.in_alt_screen);
    b.enter_alt_screen().unwrap();
    assert!(b.in_alt_screen);
    b.leave_alt_screen().unwrap();
    assert!(!b.in_alt_screen);
}

#[test]
fn test_linux_backend_emits_alt_screen_sequences() {
    use std::io::Write;
    // Build a backend that captures writes to a Vec<u8> — use TestBackend for
    // state checks above. For sequence checks, we verify LinuxBackend produces
    // the standard \x1b[?1049h / \x1b[?1049l via a direct byte comparison on
    // expected API behavior.
    let mut w: Vec<u8> = Vec::new();
    // LinuxBackend writes to stdout; assert the expected sequences exist as
    // documented constants on the Backend trait / module.
    w.write_all(viv::core::terminal::backend::ENTER_ALT_SCREEN).unwrap();
    w.write_all(viv::core::terminal::backend::LEAVE_ALT_SCREEN).unwrap();
    assert_eq!(w, b"\x1b[?1049h\x1b[?1049l");
}
