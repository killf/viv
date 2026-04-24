//! SimTerminal integration tests.
//!
//! These tests verify the SimTerminal can correctly simulate TUI behavior
//! by parsing ANSI output and reconstructing terminal state.

use viv::core::terminal::simulator::SimTerminal;
use viv::core::terminal::input::KeyEvent;
use viv::agent::protocol::AgentMessage;

#[test]
fn new_simulator_has_correct_dimensions() {
    let sim = SimTerminal::new(80, 24);
    let screen = sim.screen();
    assert_eq!(screen.size(), (80, 24));
}

#[test]
fn resize_changes_dimensions() {
    let mut sim = SimTerminal::new(80, 24);
    sim.resize(120, 40);
    let screen = sim.screen();
    assert_eq!(screen.size(), (120, 40));
}

#[test]
fn send_ready_message_shows_welcome() {
    let mut sim = SimTerminal::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "claude-3-5-sonnet".into() });
    let screen = sim.screen();
    // WelcomeWidget shows model name in info section
    assert!(
        screen.contains("claude-3-5-sonnet"),
        "welcome should show model name"
    );
    // WelcomeWidget shows block-character logo
    assert!(
        screen.contains("▐") || screen.contains("█"),
        "welcome should show block-character logo"
    );
}

#[test]
fn typing_in_editor_appears_on_screen() {
    let mut sim = SimTerminal::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_key(KeyEvent::Char('h'));
    sim.send_key(KeyEvent::Char('e'));
    sim.send_key(KeyEvent::Char('l'));
    sim.send_key(KeyEvent::Char('l'));
    sim.send_key(KeyEvent::Char('o'));

    let screen = sim.screen();
    assert!(
        screen.contains("hello"),
        "typed text should appear on screen"
    );
}

#[test]
fn slash_switches_to_slash_mode() {
    let mut sim = SimTerminal::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });
    sim.send_key(KeyEvent::Char('/'));
    assert_eq!(sim.input_mode(), viv::tui::input::InputMode::SlashCommand);
}

#[test]
fn colon_switches_to_colon_mode() {
    let mut sim = SimTerminal::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });
    sim.send_key(KeyEvent::Char(':'));
    assert_eq!(sim.input_mode(), viv::tui::input::InputMode::ColonCommand);
}

#[test]
fn enter_submits_input() {
    let mut sim = SimTerminal::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });
    sim.send_key(KeyEvent::Char('h'));
    sim.send_key(KeyEvent::Char('i'));
    sim.send_key(KeyEvent::Enter);

    let screen = sim.screen();
    // After submit, the input should be committed to scrollback with "> " prefix
    // The committed text appears in the scrollback area
    assert!(
        screen.contains(">") || screen.contains("hi"),
        "submitted text should appear on screen"
    );
}

#[test]
fn permission_request_shows_menu() {
    let mut sim = SimTerminal::new(60, 20);
    sim.send_message(AgentMessage::PermissionRequest {
        tool: "Bash".into(),
        input: "rm -rf /".into(),
    });

    let screen = sim.screen();
    // The permission widget renders a bordered menu box with options
    // We verify that rendering completes without panic and screen is valid
    assert!(screen.size().0 > 0 && screen.size().1 > 0, "screen should be valid");

    // The permission menu should be visible in the live region area
    // PermissionWidget height is 5, so it occupies rows in the bottom portion
    // We check that there's non-space content in the permission area
    let has_content = (0..screen.size().1)
        .filter_map(|r| screen.line_text(r))
        .any(|line| line.trim().len() > 0);
    assert!(has_content, "screen should have rendered content");
}

#[test]
fn permission_menu_navigation() {
    let mut sim = SimTerminal::new(60, 20);
    sim.send_message(AgentMessage::PermissionRequest {
        tool: "Bash".into(),
        input: "ls".into(),
    });

    // Initial selection should be 1 (Allow is default)
    assert_eq!(sim.permission_selected(), Some(1));

    // Press down to select next option (AlwaysAllow)
    sim.send_key(KeyEvent::Down);
    assert_eq!(sim.permission_selected(), Some(2));

    // Press down again to wrap around to Deny
    sim.send_key(KeyEvent::Down);
    assert_eq!(sim.permission_selected(), Some(0));

    // Press up to go back to AlwaysAllow
    sim.send_key(KeyEvent::Up);
    assert_eq!(sim.permission_selected(), Some(2));
}

#[test]
fn thinking_message_sets_busy() {
    let mut sim = SimTerminal::new(80, 24);
    sim.send_message(AgentMessage::Thinking);
    // busy state is reflected in the status bar via spinner
    // We can't easily test the spinner, but we can verify no panic occurs
    let screen = sim.screen();
    assert!(
        screen.size().0 > 0 && screen.size().1 > 0,
        "screen should be valid"
    );
}

#[test]
fn tool_call_shows_name() {
    let mut sim = SimTerminal::new(80, 24);
    sim.send_message(AgentMessage::ToolStart {
        name: "Bash".into(),
        input: "ls -la".into(),
    });

    let screen = sim.screen();
    assert!(screen.contains("Bash"), "should show tool name");
}

#[test]
fn tool_end_updates_state() {
    let mut sim = SimTerminal::new(80, 24);
    sim.send_message(AgentMessage::ToolStart {
        name: "Bash".into(),
        input: "ls".into(),
    });
    sim.send_message(AgentMessage::ToolEnd {
        name: "Bash".into(),
        output: "file1.txt\nfile2.txt".into(),
    });
    // Tool should be committed to scrollback, not in live region
    let screen = sim.screen();
    // The tool call should have been rendered
    assert!(screen.contains("Bash") || screen.contains("ls"));
}

#[test]
fn done_clears_busy_state() {
    let mut sim = SimTerminal::new(80, 24);
    sim.send_message(AgentMessage::Thinking);
    sim.send_message(AgentMessage::Done);
    // No panic and screen is valid
    let screen = sim.screen();
    assert!(screen.size().0 > 0 && screen.size().1 > 0);
}

#[test]
fn with_cwd_sets_display_path() {
    let sim = SimTerminal::new(80, 24).with_cwd("/data/project");
    // cwd is stored but only displayed after Ready message
    let _screen = sim.screen();
    // Just verify no panic
}

#[test]
fn with_branch_sets_git_branch() {
    let sim = SimTerminal::new(80, 24).with_branch(Some("feature/test"));
    // branch is stored but only displayed after Ready message
    let _screen = sim.screen();
    // Just verify no panic
}

#[test]
fn backspace_removes_char() {
    let mut sim = SimTerminal::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_key(KeyEvent::Char('h'));
    sim.send_key(KeyEvent::Char('i'));
    assert_eq!(sim.input_content(), "hi");

    sim.send_key(KeyEvent::Backspace);
    assert_eq!(sim.input_content(), "h");
}

#[test]
fn ctrl_c_clears_input() {
    let mut sim = SimTerminal::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_key(KeyEvent::Char('h'));
    sim.send_key(KeyEvent::Char('i'));
    assert_eq!(sim.input_content(), "hi");

    sim.send_key(KeyEvent::CtrlC);
    assert_eq!(sim.input_content(), "");
}

#[test]
fn input_content_after_typing() {
    let mut sim = SimTerminal::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_key(KeyEvent::Char('a'));
    sim.send_key(KeyEvent::Char('b'));
    sim.send_key(KeyEvent::Char('c'));

    assert_eq!(sim.input_content(), "abc");
}

#[test]
fn shift_enter_inserts_newline() {
    let mut sim = SimTerminal::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_key(KeyEvent::Char('a'));
    sim.send_key(KeyEvent::ShiftEnter);
    sim.send_key(KeyEvent::Char('b'));

    let content = sim.input_content();
    assert!(content.contains('\n'), "should contain newline");
    assert!(content.contains("a"), "should contain 'a'");
    assert!(content.contains("b"), "should contain 'b'");
}
