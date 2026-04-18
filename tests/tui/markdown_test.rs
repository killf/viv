use viv::core::terminal::buffer::{Buffer, Rect};
use viv::tui::content::parse_markdown;
use viv::tui::markdown::{MarkdownBlockWidget, render_markdown};
use viv::tui::widget::Widget;

// ── MarkdownBlockWidget tests ─────────────────────────────────────────────────

#[test]
fn renders_heading_bold() {
    let nodes = parse_markdown("# Hello");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // The heading text starts at (0,0); it should be bold
    let cell = buf.get(0, 0);
    assert!(cell.bold, "heading first char should be bold");
}

#[test]
fn renders_bullet_list() {
    let nodes = parse_markdown("- item");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // Scan row 0 for the bullet character '•'
    let has_bullet = (0..area.width).any(|x| buf.get(x, 0).ch == '\u{2022}');
    assert!(has_bullet, "unordered list should render '•' in the row");
}

#[test]
fn renders_inline_code_with_color() {
    let nodes = parse_markdown("use `cargo`");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // Find the 'c' of "cargo" and check it has a non-None fg color
    let code_cell = (0..area.width).map(|x| buf.get(x, 0)).find(|c| c.ch == 'c');
    let cell = code_cell.expect("should find 'c' from 'cargo'");
    assert!(
        cell.fg.is_some(),
        "inline code should have a foreground color"
    );
    // The color should be the CLAUDE orange Rgb(215,119,87)
    use viv::core::terminal::style::Color;
    assert_eq!(cell.fg, Some(Color::Rgb(215, 119, 87)));
}

#[test]
fn renders_quote_with_bar() {
    let nodes = parse_markdown("> quoted");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // First character in row 0 should be '│'
    assert_eq!(buf.get(0, 0).ch, '\u{2502}', "quote should start with '│'");
}

#[test]
fn renders_horizontal_rule() {
    let nodes = parse_markdown("---");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // First character in row 0 should be '─'
    assert_eq!(
        buf.get(0, 0).ch,
        '\u{2500}',
        "horizontal rule should start with '─'"
    );
}

#[test]
fn height_calculation() {
    // heading(1) + paragraph(1) + list with 2 items(2) + hr(1) = 5
    let nodes = parse_markdown("# Title\nSome text\n- one\n- two\n---");
    let h = MarkdownBlockWidget::height(&nodes, 80);
    assert_eq!(h, 5, "height should be sum of node heights");
}

// ── render_markdown backward compat tests ─────────────────────────────────────

#[test]
fn render_markdown_compat() {
    let lines = render_markdown("hello **world**");
    assert!(
        !lines.is_empty(),
        "render_markdown should return non-empty Vec<Line>"
    );
    // The bold span should be present
    let has_bold = lines[0].spans.iter().any(|s| s.bold);
    assert!(has_bold, "**world** should be bold");
}

#[test]
fn bold_text_is_rendered_bold() {
    let lines = render_markdown("hello **world** end");
    let spans = &lines[0].spans;
    let bold_span = spans.iter().find(|s| s.text.contains("world")).unwrap();
    assert!(bold_span.bold, "**world** should render as bold");
}

#[test]
fn inline_code_uses_claude_color() {
    use viv::core::terminal::style::Color;
    let lines = render_markdown("use `cargo test` to run");
    let code_span = lines[0]
        .spans
        .iter()
        .find(|s| s.text.contains("cargo test"))
        .unwrap();
    assert_eq!(
        code_span.fg,
        Some(Color::Rgb(215, 119, 87)),
        "inline code should use CLAUDE orange color"
    );
}

#[test]
fn heading_h1_is_bold() {
    let lines = render_markdown("# Hello");
    let has_bold = lines[0].spans.iter().any(|s| s.bold);
    assert!(has_bold, "# heading should be bold");
}

#[test]
fn unordered_list_item_gets_bullet() {
    let lines = render_markdown("- item one");
    let text: String = lines[0].spans.iter().map(|s| s.text.as_str()).collect();
    assert!(text.contains('\u{2022}'), "- list item should render as •");
}

#[test]
fn ordered_list_item_keeps_number() {
    let lines = render_markdown("1. first item");
    let text: String = lines[0].spans.iter().map(|s| s.text.as_str()).collect();
    assert!(text.contains("1."), "ordered list item should keep number");
}

#[test]
fn fenced_code_block_content_is_rendered() {
    let md = "```\nlet x = 1;\n```";
    let lines = render_markdown(md);
    let has_code = lines
        .iter()
        .any(|l| l.spans.iter().any(|s| s.text.contains("let x = 1;")));
    assert!(has_code, "fenced code block content should be rendered");
}

#[test]
fn plain_text_passes_through() {
    let lines = render_markdown("just plain text");
    let text: String = lines[0].spans.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(text, "just plain text");
}

#[test]
fn unclosed_bold_marker_does_not_drop_content() {
    let lines = render_markdown("see **code");
    let text: String = lines[0].spans.iter().map(|s| s.text.as_str()).collect();
    assert!(
        text.contains("code"),
        "content after unclosed ** should not be dropped"
    );
}

#[test]
fn empty_input_returns_one_line() {
    let lines = render_markdown("");
    assert!(
        !lines.is_empty(),
        "empty input should return at least one line"
    );
}
