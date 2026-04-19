use viv::core::terminal::buffer::{Buffer, Rect};
use viv::tui::code_block::CodeBlockWidget;
use viv::tui::widget::Widget;

fn make_buf(width: u16, height: u16) -> Buffer {
    Buffer::empty(Rect::new(0, 0, width, height))
}

#[test]
fn code_block_renders_border() {
    let widget = CodeBlockWidget::new("let x = 1;", Some("rust"));
    let mut buf = make_buf(20, 5);
    widget.render(Rect::new(0, 0, 20, 5), &mut buf);
    // Top-left corner should be rounded '╭'
    assert_eq!(buf.get(0, 0).ch, '╭');
}

#[test]
fn code_block_renders_language_label() {
    let widget = CodeBlockWidget::new("let x = 1;", Some("rust"));
    let mut buf = make_buf(20, 5);
    widget.render(Rect::new(0, 0, 20, 5), &mut buf);
    // Top border row should contain "rust"
    let top_row: String = (0..20).map(|x| buf.get(x, 0).ch).collect();
    assert!(
        top_row.contains("rust"),
        "top row '{top_row}' should contain 'rust'"
    );
}

#[test]
fn code_block_renders_code_content() {
    let widget = CodeBlockWidget::new("hello", None);
    let mut buf = make_buf(20, 5);
    widget.render(Rect::new(0, 0, 20, 5), &mut buf);
    // Code text appears on row 1 (inside border), starting at x=1
    let row1: String = (1..19).map(|x| buf.get(x, 1).ch).collect();
    assert!(
        row1.contains("hello"),
        "row 1 '{row1}' should contain 'hello'"
    );
}

#[test]
fn code_block_height_calculation() {
    let code = "line1\nline2\nline3";
    let height = CodeBlockWidget::height(code, 80);
    // 3 lines + 2 borders = 5
    assert_eq!(height, 5);
}

#[test]
fn code_block_keyword_gets_color() {
    // "fn" is a Rust keyword, should get a non-None fg color
    let widget = CodeBlockWidget::new("fn main()", Some("rust"));
    let mut buf = make_buf(20, 5);
    widget.render(Rect::new(0, 0, 20, 5), &mut buf);
    // First content cell at (1, 1) is the 'f' of "fn"
    let cell = buf.get(1, 1);
    assert!(
        cell.fg.is_some(),
        "keyword 'fn' should have a foreground color set"
    );
}

#[test]
fn code_block_empty_code() {
    // Empty string → 1 empty line + 2 borders = height 3
    let height = CodeBlockWidget::height("", 80);
    assert_eq!(height, 3);
}

#[test]
fn code_block_inner_has_background() {
    use viv::core::terminal::style::Color;
    let widget = CodeBlockWidget::new("let x = 1;", Some("rust"));
    let mut buf = make_buf(20, 5);
    widget.render(Rect::new(0, 0, 20, 5), &mut buf);
    // Inner cell at (1, 1) should have dark background
    let cell = buf.get(1, 1);
    assert_eq!(
        cell.bg,
        Some(Color::Rgb(30, 30, 30)),
        "code block inner should have dark bg"
    );
}

#[test]
fn code_block_no_language() {
    let widget = CodeBlockWidget::new("some code", None);
    let mut buf = make_buf(20, 5);
    widget.render(Rect::new(0, 0, 20, 5), &mut buf);
    // Top border should not contain any language label — just border chars and spaces
    // The title should be absent, so the top row after '╭' at (1..right-1) should all be '─'
    let cell_1 = buf.get(1, 0);
    // If no language, position 1 on top row should be '─' (part of border line)
    assert_eq!(
        cell_1.ch, '─',
        "no language: top row col 1 should be '─', got '{}'",
        cell_1.ch
    );
}
