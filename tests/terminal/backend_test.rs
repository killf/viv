use viv::terminal::backend::*;
use viv::terminal::size::TermSize;

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
