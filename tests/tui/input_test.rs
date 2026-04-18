use viv::core::terminal::buffer::{Buffer, Rect};
use viv::tui::input::*;
use viv::tui::widget::Widget;

#[test]
fn renders_prompt_and_content() {
    let w = InputWidget::new("hello", 5, "> ");
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
    w.render(Rect::new(0, 0, 20, 1), &mut buf);
    assert_eq!(buf.get(0, 0).ch, '>');
    assert_eq!(buf.get(1, 0).ch, ' ');
    assert_eq!(buf.get(2, 0).ch, 'h');
    assert_eq!(buf.get(6, 0).ch, 'o');
}

#[test]
fn cursor_position_at_end() {
    let w = InputWidget::new("hello", 5, "> ");
    let area = Rect::new(0, 0, 20, 1);
    let (col, row) = w.cursor_position(area);
    assert_eq!(col, 7); // 2 (prompt) + 5 (chars)
    assert_eq!(row, 0);
}

#[test]
fn cursor_position_at_start() {
    let w = InputWidget::new("hello", 0, "> ");
    let (col, row) = w.cursor_position(Rect::new(0, 0, 20, 1));
    assert_eq!(col, 2); // just after prompt
    assert_eq!(row, 0);
}

#[test]
fn cursor_position_with_area_offset() {
    let w = InputWidget::new("hi", 2, "> ");
    let (col, row) = w.cursor_position(Rect::new(5, 3, 20, 1));
    assert_eq!(col, 5 + 2 + 2); // area.x + prompt + cursor
    assert_eq!(row, 3);
}

#[test]
fn prompt_color() {
    let w = InputWidget::new("hi", 0, "> ").prompt_fg(viv::core::terminal::style::Color::Ansi(32));
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
    w.render(Rect::new(0, 0, 20, 1), &mut buf);
    assert_eq!(
        buf.get(0, 0).fg,
        Some(viv::core::terminal::style::Color::Ansi(32))
    );
    assert_eq!(buf.get(2, 0).fg, None); // content has no color
}

#[test]
fn content_clips_at_right_edge() {
    // Area is 10 wide, prompt is 2, so 8 chars visible
    // Content is "abcdefghij" (10 chars), cursor at end (10)
    let w = InputWidget::new("abcdefghij", 10, "> ");
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
    w.render(Rect::new(0, 0, 10, 1), &mut buf);
    // Clips at right edge: prompt "> " (2 chars) + "abcdefgh" (8 chars) = 10 cols
    // The first 8 chars of content fit; 'i' and 'j' are clipped
    assert_eq!(buf.get(2, 0).ch, 'a');
    assert_eq!(buf.get(9, 0).ch, 'h');
}

#[test]
fn empty_content() {
    let w = InputWidget::new("", 0, "> ");
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
    w.render(Rect::new(0, 0, 20, 1), &mut buf);
    assert_eq!(buf.get(0, 0).ch, '>');
    assert_eq!(buf.get(2, 0).ch, ' '); // empty content, just spaces
}

#[test]
fn placeholder_shown_when_content_empty() {
    let w = InputWidget::new("", 0, "> ").placeholder(Some("How can I help you?"));
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 1));
    w.render(Rect::new(0, 0, 30, 1), &mut buf);
    // After the prompt "> " (col 0-1), placeholder text starts at col 2
    assert_eq!(buf.get(2, 0).ch, 'H');
}

#[test]
fn placeholder_hidden_when_content_present() {
    let w = InputWidget::new("x", 1, "> ").placeholder(Some("How can I help you?"));
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 1));
    w.render(Rect::new(0, 0, 30, 1), &mut buf);
    assert_eq!(buf.get(2, 0).ch, 'x');
}

#[test]
fn placeholder_is_dim_colored() {
    let w = InputWidget::new("", 0, "> ").placeholder(Some("hint"));
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
    w.render(Rect::new(0, 0, 20, 1), &mut buf);
    assert_eq!(
        buf.get(2, 0).fg,
        Some(viv::core::terminal::style::theme::DIM)
    );
}

#[test]
fn renders_multiline_content() {
    // Content with newline: "ab\ncd"
    let w = InputWidget::new("ab\ncd", 5, "> ");
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 2));
    w.render(Rect::new(0, 0, 20, 2), &mut buf);
    // Row 0: "> ab" — prompt at 0-1, 'a' at 2, 'b' at 3
    assert_eq!(buf.get(0, 0).ch, '>');
    assert_eq!(buf.get(2, 0).ch, 'a');
    assert_eq!(buf.get(3, 0).ch, 'b');
    // Row 1: "  cd" — indented by prompt_width=2, 'c' at 2, 'd' at 3
    assert_eq!(buf.get(2, 1).ch, 'c');
    assert_eq!(buf.get(3, 1).ch, 'd');
}

#[test]
fn multiline_cursor_position_second_row() {
    // Content "ab\nc", cursor at byte 4 (after 'c': 'a'=0,'b'=1,'\n'=2,'c'=3, end=4)
    let w = InputWidget::new("ab\nc", 4, "> ");
    let (col, row) = w.cursor_position(Rect::new(0, 0, 20, 2));
    assert_eq!(row, 1);
    assert_eq!(col, 2 + 1); // prompt_width(2) + 1 char 'c'
}
