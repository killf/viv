use viv::core::terminal::buffer::*;
use viv::core::terminal::style::Color;

#[test]
fn rect_new_and_accessors() {
    let r = Rect::new(1, 2, 10, 5);
    assert_eq!((r.x, r.y, r.width, r.height), (1, 2, 10, 5));
}

#[test]
fn rect_area_and_empty() {
    assert_eq!(Rect::new(0, 0, 10, 5).area(), 50);
    assert!(Rect::new(0, 0, 0, 5).is_empty());
    assert!(!Rect::new(0, 0, 1, 1).is_empty());
}

#[test]
fn rect_split_vertical() {
    let r = Rect::new(0, 0, 80, 24);
    let (top, bottom) = r.split_vertical(10);
    assert_eq!(top, Rect::new(0, 0, 80, 10));
    assert_eq!(bottom, Rect::new(0, 10, 80, 14));
}

#[test]
fn rect_split_horizontal() {
    let r = Rect::new(0, 0, 80, 24);
    let (left, right) = r.split_horizontal(30);
    assert_eq!(left, Rect::new(0, 0, 30, 24));
    assert_eq!(right, Rect::new(30, 0, 50, 24));
}

#[test]
fn rect_inner_shrinks() {
    let r = Rect::new(5, 5, 20, 10);
    let i = r.inner();
    assert_eq!(i, Rect::new(6, 6, 18, 8));
}

#[test]
fn rect_inner_of_tiny_is_empty() {
    let r = Rect::new(0, 0, 1, 1);
    assert!(r.inner().is_empty());
}

#[test]
fn buffer_empty_all_default() {
    let buf = Buffer::empty(Rect::new(0, 0, 5, 3));
    for y in 0..3 {
        for x in 0..5 {
            assert_eq!(*buf.get(x, y), Cell::default());
        }
    }
}

#[test]
fn buffer_set_get() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 5));
    let cell = Cell { ch: 'X', fg: Some(Color::Ansi(31)), bg: None, bold: true };
    buf.set(3, 2, cell);
    assert_eq!(*buf.get(3, 2), cell);
}

#[test]
fn buffer_set_str() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
    buf.set_str(2, 1, "Hello", Some(Color::Ansi(32)), false);
    assert_eq!(buf.get(2, 1).ch, 'H');
    assert_eq!(buf.get(3, 1).ch, 'e');
    assert_eq!(buf.get(6, 1).ch, 'o');
    assert_eq!(buf.get(2, 1).fg, Some(Color::Ansi(32)));
}

#[test]
fn buffer_set_str_rgb_color() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
    let claude = Color::Rgb(215, 119, 87);
    buf.set_str(0, 0, "hi", Some(claude), false);
    assert_eq!(buf.get(0, 0).fg, Some(claude));
    assert_eq!(buf.get(1, 0).fg, Some(claude));
}

#[test]
fn buffer_diff_emits_rgb_sequence() {
    let mut current = Buffer::empty(Rect::new(0, 0, 5, 1));
    let previous = Buffer::empty(Rect::new(0, 0, 5, 1));
    current.set_str(0, 0, "X", Some(Color::Rgb(215, 119, 87)), false);
    let diff = current.diff(&previous);
    let s = String::from_utf8_lossy(&diff);
    assert!(s.contains("\x1b[38;2;215;119;87m"));
    assert!(s.contains('X'));
}

#[test]
fn buffer_set_str_clips() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 5, 1));
    buf.set_str(3, 0, "Hello", None, false);
    assert_eq!(buf.get(3, 0).ch, 'H');
    assert_eq!(buf.get(4, 0).ch, 'e');
    // "llo" clipped
}

#[test]
fn buffer_diff_identical_empty() {
    let a = Buffer::empty(Rect::new(0, 0, 10, 5));
    let b = Buffer::empty(Rect::new(0, 0, 10, 5));
    assert!(a.diff(&b).is_empty());
}

#[test]
fn buffer_diff_detects_change() {
    let mut current = Buffer::empty(Rect::new(0, 0, 10, 5));
    let previous = Buffer::empty(Rect::new(0, 0, 10, 5));
    current.set_char(0, 0, 'A');
    let diff = current.diff(&previous);
    assert!(!diff.is_empty());
    let s = String::from_utf8_lossy(&diff);
    assert!(s.contains('A'));
}

#[test]
fn buffer_clear_resets() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 5, 3));
    buf.set_char(0, 0, 'X');
    buf.clear();
    assert_eq!(buf.get(0, 0).ch, ' ');
}
