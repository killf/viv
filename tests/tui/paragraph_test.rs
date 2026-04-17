use viv::tui::paragraph::*;
use viv::tui::widget::Widget;
use viv::core::terminal::buffer::{Rect, Buffer};

#[test]
fn single_line_renders() {
    let p = Paragraph::new(vec![Line::raw("hello")]);
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
    p.render(Rect::new(0, 0, 20, 5), &mut buf);
    assert_eq!(buf.get(0, 0).ch, 'h');
    assert_eq!(buf.get(4, 0).ch, 'o');
}

#[test]
fn wraps_long_line() {
    let p = Paragraph::new(vec![Line::raw("hello world")]);
    let mut buf = Buffer::empty(Rect::new(0, 0, 6, 5));
    p.render(Rect::new(0, 0, 6, 5), &mut buf);
    // "hello " on row 0, "world" on row 1
    assert_eq!(buf.get(0, 0).ch, 'h');
    assert_eq!(buf.get(0, 1).ch, 'w');
}

#[test]
fn scroll_skips_lines() {
    let p = Paragraph::new(vec![
        Line::raw("line0"),
        Line::raw("line1"),
        Line::raw("line2"),
    ]).scroll(1);
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
    p.render(Rect::new(0, 0, 20, 5), &mut buf);
    // line0 skipped, line1 at row 0
    assert_eq!(buf.get(0, 0).ch, 'l');
    assert_eq!(buf.get(4, 0).ch, '1');
}

#[test]
fn styled_spans() {
    let line = Line::from_spans(vec![
        Span::styled("red", viv::core::terminal::style::Color::Ansi(31), false),
        Span::raw(" normal"),
    ]);
    let p = Paragraph::new(vec![line]);
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
    p.render(Rect::new(0, 0, 20, 5), &mut buf);
    assert_eq!(buf.get(0, 0).fg, Some(viv::core::terminal::style::Color::Ansi(31)));
    assert_eq!(buf.get(0, 0).ch, 'r');
    assert_eq!(buf.get(4, 0).fg, None); // space after "red" is unstyled
}

#[test]
fn empty_renders_nothing() {
    let p = Paragraph::new(vec![]);
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
    p.render(Rect::new(0, 0, 20, 5), &mut buf);
    assert_eq!(buf.get(0, 0).ch, ' ');
}

#[test]
fn clips_at_area_bottom() {
    let lines: Vec<Line> = (0..100).map(|i| Line::raw(format!("line{}", i))).collect();
    let p = Paragraph::new(lines);
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
    p.render(Rect::new(0, 0, 20, 3), &mut buf);
    // Only 3 rows rendered
    assert_eq!(buf.get(0, 0).ch, 'l'); // line0
    assert_eq!(buf.get(0, 2).ch, 'l'); // line2
}

#[test]
fn multi_word_wrap() {
    let p = Paragraph::new(vec![Line::raw("the quick brown fox jumps")]);
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 5));
    p.render(Rect::new(0, 0, 10, 5), &mut buf);
    // "the quick " (10 chars) on row 0, "brown fox " on row 1, "jumps" on row 2
    assert_eq!(buf.get(0, 0).ch, 't');
    assert_eq!(buf.get(0, 1).ch, 'b');
}
