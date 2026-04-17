use viv::tui::markdown::render_markdown;
use viv::terminal::style::theme;

#[test]
fn bold_text_is_rendered_bold() {
    let lines = render_markdown("hello **world** end");
    let spans = &lines[0].spans;
    let bold_span = spans.iter().find(|s| s.text.contains("world")).unwrap();
    assert!(bold_span.bold, "**world** should render as bold");
}

#[test]
fn inline_code_uses_suggestion_color() {
    let lines = render_markdown("use `cargo test` to run");
    let code_span = lines[0].spans.iter().find(|s| s.text.contains("cargo test")).unwrap();
    assert_eq!(code_span.fg, Some(theme::SUGGESTION));
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
    assert!(text.contains('•'), "- list item should render as •");
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
    let has_code = lines.iter().any(|l| {
        l.spans.iter().any(|s| s.text.contains("let x = 1;"))
    });
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
    // "code" must not be dropped
    assert!(text.contains("code"), "content after unclosed ** should not be dropped");
}

#[test]
fn empty_input_returns_one_line() {
    let lines = render_markdown("");
    assert!(!lines.is_empty(), "empty input should return at least one line");
}
