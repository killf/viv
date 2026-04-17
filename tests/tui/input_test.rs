use viv::tui::input::*;
use viv::tui::widget::Widget;
use viv::terminal::buffer::{Rect, Buffer};

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
    let w = InputWidget::new("hi", 0, "> ").prompt_fg(viv::terminal::style::Color::Ansi(32));
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
    w.render(Rect::new(0, 0, 20, 1), &mut buf);
    assert_eq!(buf.get(0, 0).fg, Some(viv::terminal::style::Color::Ansi(32)));
    assert_eq!(buf.get(2, 0).fg, None); // content has no color
}

#[test]
fn scrolls_when_content_exceeds_width() {
    // Area is 10 wide, prompt is 2, so 8 chars visible
    // Content is "abcdefghij" (10 chars), cursor at end (10)
    let w = InputWidget::new("abcdefghij", 10, "> ");
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
    w.render(Rect::new(0, 0, 10, 1), &mut buf);
    // Should show the tail end with cursor visible
    // The last visible chars should include 'j'
    let mut found_j = false;
    for x in 0..10 {
        if buf.get(x, 0).ch == 'j' { found_j = true; }
    }
    assert!(found_j, "Should scroll to show cursor position with 'j' visible");
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
    let w = InputWidget::new("", 0, "> ")
        .placeholder(Some("How can I help you?"));
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 1));
    w.render(Rect::new(0, 0, 30, 1), &mut buf);
    // After the prompt "> " (col 0-1), placeholder text starts at col 2
    assert_eq!(buf.get(2, 0).ch, 'H');
}

#[test]
fn placeholder_hidden_when_content_present() {
    let w = InputWidget::new("x", 1, "> ")
        .placeholder(Some("How can I help you?"));
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 1));
    w.render(Rect::new(0, 0, 30, 1), &mut buf);
    assert_eq!(buf.get(2, 0).ch, 'x');
}

#[test]
fn placeholder_is_dim_colored() {
    let w = InputWidget::new("", 0, "> ")
        .placeholder(Some("hint"));
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
    w.render(Rect::new(0, 0, 20, 1), &mut buf);
    assert_eq!(buf.get(2, 0).fg, Some(viv::terminal::style::theme::DIM));
}
