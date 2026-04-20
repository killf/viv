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
    // Find the 'c' of "cargo" and check it has the new inline code color
    let code_cell = (0..area.width).map(|x| buf.get(x, 0)).find(|c| c.ch == 'c');
    let cell = code_cell.expect("should find 'c' from 'cargo'");
    assert!(
        cell.fg.is_some(),
        "inline code should have a foreground color"
    );
    use viv::core::terminal::style::Color;
    assert_eq!(cell.fg, Some(Color::Rgb(230, 150, 100)));
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
    // heading(1) + spacing(1) + paragraph(1) + spacing(1) + list(2) + spacing(1) + hr(1) = 8
    let nodes = parse_markdown("# Title\nSome text\n- one\n- two\n---");
    let h = MarkdownBlockWidget::height(&nodes, 80);
    assert_eq!(h, 8, "height should include spacing between nodes");
}

#[test]
fn paragraph_wraps_long_text() {
    let nodes = parse_markdown("hello world");
    let h = MarkdownBlockWidget::height(&nodes, 6);
    assert_eq!(h, 2, "long paragraph should wrap to 2 rows in width 6");
}

#[test]
fn paragraph_renders_wrapped_second_row() {
    let nodes = parse_markdown("hello world");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 6, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    assert_eq!(buf.get(0, 0).ch, 'h');
    assert_eq!(buf.get(0, 1).ch, 'w', "wrapped word should appear on row 1");
}

#[test]
fn block_spacing_between_nodes() {
    let nodes = parse_markdown("Hello\n\nWorld");
    let h = MarkdownBlockWidget::height(&nodes, 80);
    assert_eq!(h, 3, "two paragraphs should have 1 row spacing: 1+1+1=3");
}

#[test]
fn list_item_wraps_long_text() {
    let nodes = parse_markdown("- hello world");
    let h = MarkdownBlockWidget::height(&nodes, 10);
    assert_eq!(h, 2, "long list item should wrap");
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
fn inline_code_uses_new_orange_color() {
    use viv::core::terminal::style::Color;
    let lines = render_markdown("use `cargo test` to run");
    let code_span = lines[0]
        .spans
        .iter()
        .find(|s| s.text.contains("cargo test"))
        .unwrap();
    assert_eq!(
        code_span.fg,
        Some(Color::Rgb(230, 150, 100)),
        "inline code should use new orange color"
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

// ── Visual styling tests ─────────────────────────────────────────────────────

#[test]
fn heading_h1_uses_claude_color() {
    use viv::core::terminal::style::Color;
    let nodes = parse_markdown("# Title");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let cell = buf.get(0, 0);
    assert_eq!(
        cell.fg,
        Some(Color::Rgb(215, 119, 87)),
        "h1 should use CLAUDE orange"
    );
    assert!(cell.bold, "h1 should be bold");
}

#[test]
fn heading_h3_uses_dim_color() {
    use viv::core::terminal::style::Color;
    let nodes = parse_markdown("### Subtitle");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let cell = buf.get(0, 0);
    assert_eq!(
        cell.fg,
        Some(Color::Rgb(136, 136, 136)),
        "h3 should use DIM gray"
    );
    assert!(cell.bold, "h3 should be bold");
}

#[test]
fn inline_code_uses_new_color() {
    use viv::core::terminal::style::Color;
    let nodes = parse_markdown("use `cargo`");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let code_cell = (0..area.width).map(|x| buf.get(x, 0)).find(|c| c.ch == 'c');
    let cell = code_cell.expect("should find 'c'");
    assert_eq!(
        cell.fg,
        Some(Color::Rgb(230, 150, 100)),
        "inline code new orange"
    );
    assert_eq!(
        cell.bg,
        Some(Color::Rgb(45, 40, 38)),
        "inline code subtle bg"
    );
}

#[test]
fn italic_uses_italic_flag() {
    let nodes = parse_markdown("*hello*");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let cell = buf.get(0, 0);
    assert!(cell.italic, "italic text should set cell.italic");
    assert_eq!(
        cell.fg,
        Some(viv::core::terminal::style::Color::Rgb(255, 255, 255)),
        "italic should use TEXT white"
    );
}

#[test]
fn quote_preserves_bold_and_adds_italic() {
    let nodes = parse_markdown("> **bold** text");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // After "\u{2502} " (│ is 2 cols, space is 1 col), bold text starts at x=3
    let bold_cell = buf.get(3, 0);
    assert!(bold_cell.bold, "bold inside quote should stay bold");
    assert!(bold_cell.italic, "quote content should be italic");
}
