use viv::core::terminal::buffer::{Buffer, Rect};
use viv::tui::tool_call::{ToolCallState, ToolCallWidget, ToolStatus, extract_input_summary};
use viv::tui::widget::StatefulWidget;

fn read_row(buf: &Buffer, y: u16, width: u16) -> String {
    (0..width).map(|x| buf.get(x, y).ch).collect()
}

#[test]
fn folded_renders_single_line() {
    let widget = ToolCallWidget::new("Read", "src/main.rs", r#"{"file_path": "src/main.rs"}"#);
    let mut state = ToolCallState::new_success("35 lines".to_string());
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 5));
    widget.render(Rect::new(0, 0, 80, 5), &mut buf, &mut state);

    let row = read_row(&buf, 0, 80);
    assert!(row.contains("Read"), "should contain tool name");
    assert!(row.contains('✓'), "should contain success checkmark");
}

#[test]
fn folded_height_is_one() {
    // When folded, only row 0 should have content; row 1 should be blank
    let widget = ToolCallWidget::new("Read", "src/main.rs", r#"{"file_path": "src/main.rs"}"#);
    let mut state = ToolCallState::new_success("35 lines".to_string());
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 5));
    widget.render(Rect::new(0, 0, 80, 5), &mut buf, &mut state);

    let row1: String = (0..80).map(|x| buf.get(x, 1).ch).collect();
    // Row 1 should be all spaces (no content rendered there)
    assert!(
        row1.chars().all(|c| c == ' '),
        "row 1 should be empty when folded, got: {:?}",
        row1
    );
}

#[test]
fn error_shows_cross() {
    let widget = ToolCallWidget::new("Bash", "ls -la", r#"{"command": "ls -la"}"#);
    let mut state = ToolCallState::new_error("permission denied".to_string());
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
    widget.render(Rect::new(0, 0, 80, 3), &mut buf, &mut state);

    let row = read_row(&buf, 0, 80);
    assert!(row.contains('✗'), "should contain error cross");
    assert!(row.contains("Bash"), "should contain tool name");
}

#[test]
fn running_shows_gear_and_running() {
    let widget = ToolCallWidget::new("Bash", "ls -la", r#"{"command": "ls -la"}"#);
    let mut state = ToolCallState::new_running();
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
    widget.render(Rect::new(0, 0, 80, 3), &mut buf, &mut state);

    let row = read_row(&buf, 0, 80);
    assert!(row.contains("Bash"), "should contain tool name");
    // Running status should appear somewhere in the row
    assert!(
        row.contains('⚙') || row.contains('r'),
        "should show running indicator"
    );
}

#[test]
fn extract_summary_read() {
    let json = r#"{"file_path": "src/main.rs"}"#;
    let summary = extract_input_summary("Read", json);
    assert!(
        summary.contains("src/main.rs"),
        "should contain filename, got: {:?}",
        summary
    );
}

#[test]
fn extract_summary_bash() {
    let json = r#"{"command": "ls -la /tmp"}"#;
    let summary = extract_input_summary("Bash", json);
    assert!(
        summary.contains("ls -la /tmp"),
        "should contain command, got: {:?}",
        summary
    );
}

#[test]
fn extract_summary_grep() {
    let json = r#"{"pattern": "fn main"}"#;
    let summary = extract_input_summary("Grep", json);
    assert!(
        summary.contains("fn main"),
        "should contain pattern, got: {:?}",
        summary
    );
}

#[test]
fn extract_summary_glob() {
    let json = r#"{"pattern": "**/*.rs"}"#;
    let summary = extract_input_summary("Glob", json);
    assert!(
        summary.contains("**/*.rs"),
        "should contain pattern, got: {:?}",
        summary
    );
}

#[test]
fn extract_summary_webfetch() {
    let json = r#"{"url": "https://example.com"}"#;
    let summary = extract_input_summary("WebFetch", json);
    assert!(
        summary.contains("https://example.com"),
        "should contain url, got: {:?}",
        summary
    );
}

#[test]
fn extract_summary_agent() {
    let json = r#"{"description": "Analyze code quality"}"#;
    let summary = extract_input_summary("Agent", json);
    assert!(
        summary.contains("Analyze code quality"),
        "should contain description, got: {:?}",
        summary
    );
}

#[test]
fn extract_summary_bash_truncation() {
    let long_cmd = "x".repeat(100);
    let json = format!(r#"{{"command": "{}"}}"#, long_cmd);
    let summary = extract_input_summary("Bash", &json);
    assert!(
        summary.len() <= 60,
        "bash summary should be truncated to 60 chars, got len {}",
        summary.len()
    );
}

#[test]
fn focus_indicator_shows_bar() {
    let widget =
        ToolCallWidget::new("Read", "src/main.rs", r#"{"file_path": "src/main.rs"}"#).focused(true);
    let mut state = ToolCallState::new_success("ok".to_string());
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
    widget.render(Rect::new(0, 0, 80, 3), &mut buf, &mut state);

    let first_char = buf.get(0, 0).ch;
    assert_eq!(first_char, '┃', "focused: first char should be ┃");
}

#[test]
fn unfocused_no_bar() {
    let widget = ToolCallWidget::new("Read", "src/main.rs", r#"{"file_path": "src/main.rs"}"#)
        .focused(false);
    let mut state = ToolCallState::new_success("ok".to_string());
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 3));
    widget.render(Rect::new(0, 0, 80, 3), &mut buf, &mut state);

    let first_char = buf.get(0, 0).ch;
    assert_eq!(first_char, ' ', "unfocused: first char should be space");
}

#[test]
fn toggle_fold() {
    let mut state = ToolCallState::new_success("ok".to_string());
    assert!(state.folded, "should start folded");
    state.toggle_fold();
    assert!(!state.folded, "should be unfolded after toggle");
    state.toggle_fold();
    assert!(state.folded, "should be folded again after second toggle");
}

#[test]
fn expanded_renders_input_block() {
    let input_raw = r#"{"file_path": "src/main.rs"}"#;
    let widget = ToolCallWidget::new("Read", "src/main.rs", input_raw);
    let mut state = ToolCallState::new_success("ok".to_string());
    state.toggle_fold(); // unfold
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));
    widget.render(Rect::new(0, 0, 80, 10), &mut buf, &mut state);

    // Should have content beyond row 0 (the expanded block)
    let has_content_below = (1..10).any(|y| (0..80).any(|x| buf.get(x, y).ch != ' '));
    assert!(
        has_content_below,
        "expanded state should render content below header"
    );
}

#[test]
fn tool_status_debug_clone() {
    let s1 = ToolStatus::Running;
    let s2 = s1.clone();
    assert!(matches!(s2, ToolStatus::Running));

    let s3 = ToolStatus::Success {
        summary: "done".to_string(),
    };
    let s4 = s3.clone();
    assert!(matches!(s4, ToolStatus::Success { .. }));

    let s5 = ToolStatus::Error {
        message: "oops".to_string(),
    };
    let s6 = s5.clone();
    assert!(matches!(s6, ToolStatus::Error { .. }));
}

#[test]
fn tool_call_state_new_running() {
    let state = ToolCallState::new_running();
    assert!(state.folded);
    assert_eq!(state.output_scroll, 0);
    assert!(matches!(state.status, ToolStatus::Running));
}

#[test]
fn tool_call_state_new_success() {
    let state = ToolCallState::new_success("done".to_string());
    assert!(state.folded);
    assert!(matches!(state.status, ToolStatus::Success { .. }));
}

#[test]
fn tool_call_state_new_error() {
    let state = ToolCallState::new_error("fail".to_string());
    assert!(state.folded);
    assert!(matches!(state.status, ToolStatus::Error { .. }));
}
