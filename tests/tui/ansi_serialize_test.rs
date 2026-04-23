use viv::core::terminal::buffer::{Buffer, Rect};
use viv::tui::ansi_serialize::buffer_rows_to_ansi;

#[test]
fn serializes_plain_ascii_row_then_newline() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 5, 1));
    buf.set_str(0, 0, "hello", None, false);
    let out = buffer_rows_to_ansi(&buf, 0..1);
    // Row ends with reset SGR + erase-to-EOL + CRLF so the caller can redraw
    // in place without leaving stale characters past the trimmed tail.
    assert!(out.ends_with(b"\x1b[0m\x1b[K\r\n"));
    assert!(out.windows(5).any(|w| w == b"hello"));
}

#[test]
fn collapses_trailing_blanks() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
    buf.set_str(0, 0, "hi", None, false);
    let out = buffer_rows_to_ansi(&buf, 0..1);
    let body = std::str::from_utf8(&out).expect("utf8");
    assert!(
        !body.contains("          \x1b[0m\r\n"),
        "expected trailing blanks trimmed, got {:?}",
        body
    );
}

#[test]
fn emits_one_line_per_row_in_range() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 4, 3));
    buf.set_str(0, 0, "aaa", None, false);
    buf.set_str(0, 1, "bbb", None, false);
    buf.set_str(0, 2, "ccc", None, false);
    let out = buffer_rows_to_ansi(&buf, 0..3);
    let nl_count = out.iter().filter(|&&b| b == b'\n').count();
    assert_eq!(nl_count, 3);
    // Each \n must be preceded by \r so cursor returns to column 0 under raw mode.
    let cr_lf_pairs = out.windows(2).filter(|w| w == b"\r\n").count();
    assert_eq!(cr_lf_pairs, 3);
}
