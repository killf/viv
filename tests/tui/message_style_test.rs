//! Tests for ClaudeStyle message formatting helpers.
use viv::tui::message_style::*;
use viv::core::terminal::style::theme;

#[test]
fn user_message_has_angle_prefix() {
    let line = format_user_message("hello world");
    assert_eq!(line.spans.len(), 2);
    assert_eq!(line.spans[0].text, "> ");
    assert_eq!(line.spans[0].fg, Some(theme::CLAUDE));
    assert_eq!(line.spans[1].text, "hello world");
}

#[test]
fn assistant_message_first_line_has_dot_bullet() {
    // Single-line assistant response
    let lines = format_assistant_message("hi");
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].spans[0].text, "● ");
    assert_eq!(lines[0].spans[0].fg, Some(theme::CLAUDE));
    assert_eq!(lines[0].spans[1].text, "hi");
}

#[test]
fn assistant_message_continuation_uses_gutter_prefix() {
    // Multi-line response: first line has ●, others indented with padding
    let lines = format_assistant_message("line one\nline two\nline three");
    assert_eq!(lines.len(), 3);

    // First: ● + text
    assert_eq!(lines[0].spans[0].text, "● ");
    assert_eq!(lines[0].spans[1].text, "line one");

    // Continuation lines: "  " indent + text
    assert_eq!(lines[1].spans[0].text, "  ");
    assert_eq!(lines[1].spans[1].text, "line two");
    assert_eq!(lines[2].spans[0].text, "  ");
    assert_eq!(lines[2].spans[1].text, "line three");
}

#[test]
fn assistant_message_empty_input_returns_bullet_only() {
    let lines = format_assistant_message("");
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].spans[0].text, "● ");
    assert_eq!(lines[0].spans[1].text, "");
}

#[test]
fn error_message_uses_error_color() {
    let lines = format_error_message("oops");
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].spans[0].text, "● ");
    assert_eq!(lines[0].spans[0].fg, Some(theme::ERROR));
    assert_eq!(lines[0].spans[1].text, "oops");
    assert_eq!(lines[0].spans[1].fg, Some(theme::ERROR));
}

#[test]
fn welcome_line_is_single_bullet_plus_ready() {
    let line = format_welcome("", None);
    assert_eq!(line.spans[0].text, "● ");
    assert_eq!(line.spans[0].fg, Some(theme::CLAUDE));
    assert_eq!(line.spans[1].text, "viv");
    assert!(line.spans[1].bold);
    // Last span is "ready" in dim
    let last = line.spans.last().unwrap();
    assert_eq!(last.fg, Some(theme::DIM));
    assert!(last.text.contains("ready"));
}
