use viv::tui::permission::{render_permission_pending, render_permission_result};
use viv::terminal::style::theme;

#[test]
fn pending_line_starts_with_suggestion_bullet() {
    let line = render_permission_pending("Bash", "ls -la");
    // First span should be "  ◆ " in SUGGESTION color
    assert_eq!(line.spans[0].text, "  \u{25c6} ");
    assert_eq!(line.spans[0].fg, Some(theme::SUGGESTION));
}

#[test]
fn pending_line_contains_tool_name_and_summary() {
    let line = render_permission_pending("Bash", "ls -la");
    let full: String = line.spans.iter().map(|s| s.text.as_str()).collect();
    assert!(full.contains("Bash"), "should contain tool name");
    assert!(full.contains("ls -la"), "should contain summary");
}

#[test]
fn result_allowed_uses_success_color() {
    let line = render_permission_result("Bash", "ls -la", true);
    let has_success = line.spans.iter().any(|s| s.fg == Some(theme::SUCCESS));
    assert!(has_success, "allowed result should use SUCCESS color");
}

#[test]
fn result_denied_uses_error_color() {
    let line = render_permission_result("Bash", "ls -la", false);
    let has_error = line.spans.iter().any(|s| s.fg == Some(theme::ERROR));
    assert!(has_error, "denied result should use ERROR color");
}
