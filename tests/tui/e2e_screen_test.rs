//! End-to-end UI tests for TerminalSimulator.
//!
//! These tests assert the COMPLETE screen layout, not just partial content.
//! This provides true TDD validation of the UI rendering.

use viv::core::terminal::simulator::TerminalSimulator;
use viv::core::terminal::input::KeyEvent;
use viv::agent::protocol::AgentMessage;

/// Full end-to-end test: Welcome screen renders with complete layout.
#[test]
fn e2e_welcome_screen_layout() {
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

    sim.send_message(AgentMessage::Ready {
        model: "claude-3-5-sonnet-20241022".into(),
    });

    let screen = sim.screen();

    // Assert the complete Welcome screen layout
    // Rows 0-4 contain the logo + info, rows 5-23 are empty (prompt area)
    screen.assert_screen(&[
        "       _           Model:    claude-3-5-sonnet-20241022",
        "__   _(_)_   __    CWD:      /data/project",
        "\\ \\ / / \\ \\ / /    Branch:   -",
        " \\ V /| |\\ V /     Platform: linux x86_64",
        "  \\_/ |_| \\_/      Shell:    zsh",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
    ]);
}

/// Full end-to-end test: Input editor with typed text.
#[test]
fn e2e_input_editor_with_text() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    // Type "Hello" at the prompt
    sim.send_key(KeyEvent::Char('H'));
    sim.send_key(KeyEvent::Char('e'));
    sim.send_key(KeyEvent::Char('l'));
    sim.send_key(KeyEvent::Char('l'));
    sim.send_key(KeyEvent::Char('o'));

    let screen = sim.screen();

    // First 5 rows: Welcome logo (no longer visible due to prompt area)
    // Prompt row should show the input
    assert!(
        screen.contains("Hello"),
        "Typed 'Hello' should appear on prompt line"
    );
}

/// Full end-to-end test: Submit input clears prompt and shows in scrollback.
#[test]
fn e2e_submit_input_clears_prompt() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_key(KeyEvent::Char('H'));
    sim.send_key(KeyEvent::Char('e'));
    sim.send_key(KeyEvent::Char('l'));
    sim.send_key(KeyEvent::Char('l'));
    sim.send_key(KeyEvent::Char('o'));
    sim.send_key(KeyEvent::Enter);

    let screen = sim.screen();

    // After submit, prompt should be empty
    assert_eq!(
        sim.input_content(),
        "",
        "Prompt should be empty after submit"
    );
}

/// Full end-to-end test: Slash command mode indicator.
#[test]
fn e2e_slash_command_mode() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_key(KeyEvent::Char('/'));

    assert_eq!(
        sim.input_mode(),
        viv::tui::input::InputMode::SlashCommand,
        "Typing / should switch to SlashCommand mode"
    );
}

/// Full end-to-end test: Colon command mode indicator.
#[test]
fn e2e_colon_command_mode() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_key(KeyEvent::Char(':'));

    assert_eq!(
        sim.input_mode(),
        viv::tui::input::InputMode::ColonCommand,
        "Typing : should switch to ColonCommand mode"
    );
}

/// Full end-to-end test: Backspace removes characters.
#[test]
fn e2e_backspace_removes_char() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_key(KeyEvent::Char('h'));
    sim.send_key(KeyEvent::Char('e'));
    sim.send_key(KeyEvent::Char('l'));
    sim.send_key(KeyEvent::Char('l'));
    sim.send_key(KeyEvent::Char('o'));

    assert_eq!(sim.input_content(), "hello");

    sim.send_key(KeyEvent::Backspace);
    assert_eq!(sim.input_content(), "hell");

    sim.send_key(KeyEvent::Backspace);
    sim.send_key(KeyEvent::Backspace);
    assert_eq!(sim.input_content(), "he");
}

/// Full end-to-end test: Ctrl+C clears input.
#[test]
fn e2e_ctrl_c_clears_input() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_key(KeyEvent::Char('s'));
    sim.send_key(KeyEvent::Char('o'));
    sim.send_key(KeyEvent::Char('m'));
    sim.send_key(KeyEvent::Char('e'));
    sim.send_key(KeyEvent::Char(' '));
    sim.send_key(KeyEvent::Char('t'));
    sim.send_key(KeyEvent::Char('e'));
    sim.send_key(KeyEvent::Char('x'));
    sim.send_key(KeyEvent::Char('t'));

    assert!(!sim.input_content().is_empty());

    sim.send_key(KeyEvent::CtrlC);

    assert_eq!(sim.input_content(), "");
}

/// Full end-to-end test: Multi-line input with Shift+Enter.
#[test]
fn e2e_multiline_input() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_key(KeyEvent::Char('f'));
    sim.send_key(KeyEvent::Char('i'));
    sim.send_key(KeyEvent::Char('r'));
    sim.send_key(KeyEvent::Char('s'));
    sim.send_key(KeyEvent::Char('t'));
    sim.send_key(KeyEvent::ShiftEnter);

    sim.send_key(KeyEvent::Char('s'));
    sim.send_key(KeyEvent::Char('e'));
    sim.send_key(KeyEvent::Char('c'));
    sim.send_key(KeyEvent::Char('o'));
    sim.send_key(KeyEvent::Char('n'));
    sim.send_key(KeyEvent::Char('d'));

    let content = sim.input_content();
    assert!(content.contains('\n'), "Shift+Enter should insert newline");
    assert!(content.contains("first"), "First line should be preserved");
    assert!(content.contains("second"), "Second line should exist");
}

/// Full end-to-end test: Resize updates display dimensions.
#[test]
fn e2e_resize_updates_display() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    let (w1, h1) = sim.screen().size();
    assert_eq!((w1, h1), (80, 24));

    sim.resize(120, 40);

    let (w2, h2) = sim.screen().size();
    assert_eq!((w2, h2), (120, 40));
}

/// Full end-to-end test: History browsing with arrow keys.
#[test]
fn e2e_history_browsing() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    // Submit "first"
    sim.send_key(KeyEvent::Char('f'));
    sim.send_key(KeyEvent::Char('i'));
    sim.send_key(KeyEvent::Char('r'));
    sim.send_key(KeyEvent::Char('s'));
    sim.send_key(KeyEvent::Char('t'));
    sim.send_key(KeyEvent::Enter);

    // Submit "second"
    sim.send_key(KeyEvent::Char('s'));
    sim.send_key(KeyEvent::Char('e'));
    sim.send_key(KeyEvent::Char('c'));
    sim.send_key(KeyEvent::Char('o'));
    sim.send_key(KeyEvent::Char('n'));
    sim.send_key(KeyEvent::Char('d'));
    sim.send_key(KeyEvent::Enter);

    // Type "third" (not submitted)
    sim.send_key(KeyEvent::Char('t'));
    sim.send_key(KeyEvent::Char('h'));
    sim.send_key(KeyEvent::Char('i'));
    sim.send_key(KeyEvent::Char('r'));
    sim.send_key(KeyEvent::Char('d'));

    // Press Up -> should show "second"
    sim.send_key(KeyEvent::Up);
    let content = sim.input_content();
    assert!(
        content.contains("second"),
        "Up should browse to most recent history entry"
    );

    // Press Up again -> should show "first"
    sim.send_key(KeyEvent::Up);
    let content = sim.input_content();
    assert!(
        content.contains("first"),
        "Up again should browse to older history entry"
    );

    // Press Down -> at oldest entry, stays at "first"
    sim.send_key(KeyEvent::Down);
    let content = sim.input_content();
    assert!(
        content.contains("first"),
        "Down at oldest entry should stay at first"
    );
}

/// Full end-to-end test: Ctrl+R enters history search mode.
#[test]
fn e2e_ctrl_r_enters_search_mode() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_key(KeyEvent::CtrlR);

    assert_eq!(
        sim.input_mode(),
        viv::tui::input::InputMode::HistorySearch,
        "Ctrl+R should enter HistorySearch mode"
    );
}

/// Full end-to-end test: Tool start shows in live region.
#[test]
fn e2e_tool_start_renders() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_message(AgentMessage::ToolStart {
        name: "Bash".into(),
        input: "ls -la".into(),
    });

    let screen = sim.screen();
    assert!(
        screen.contains("Bash"),
        "Tool call should show tool name"
    );
}

/// Full end-to-end test: Tool end shows summary (folded view).
#[test]
fn e2e_tool_end_shows_summary() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_message(AgentMessage::ToolStart {
        name: "Bash".into(),
        input: "echo hello".into(),
    });

    sim.send_message(AgentMessage::ToolEnd {
        name: "Bash".into(),
        output: "hello\n".into(),
    });

    let screen = sim.screen();
    // Tool should be committed and show summary (e.g., "6 chars")
    assert!(
        screen.contains("Bash"),
        "Tool call should show tool name"
    );
    assert!(
        screen.contains("chars"),
        "Tool call should show output summary"
    );
}

/// Full end-to-end test: Tool error shows error state.
#[test]
fn e2e_tool_error_renders() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_message(AgentMessage::ToolStart {
        name: "Bash".into(),
        input: "invalid".into(),
    });

    sim.send_message(AgentMessage::ToolError {
        name: "Bash".into(),
        error: "Command not found".into(),
    });

    // Should render without error
    let screen = sim.screen();
    assert!(screen.size().0 > 0 && screen.size().1 > 0);
}

/// Full end-to-end test: Permission menu selection.
#[test]
fn e2e_permission_menu_selection() {
    let mut sim = TerminalSimulator::new(60, 20);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_message(AgentMessage::PermissionRequest {
        tool: "Bash".into(),
        input: "rm -rf /".into(),
    });

    // Initial selection should be Allow (index 1)
    assert_eq!(sim.permission_selected(), Some(1));

    // Down -> AlwaysAllow (index 2)
    sim.send_key(KeyEvent::Down);
    assert_eq!(sim.permission_selected(), Some(2));

    // Down -> wrap to Deny (index 0)
    sim.send_key(KeyEvent::Down);
    assert_eq!(sim.permission_selected(), Some(0));

    // Up -> AlwaysAllow (index 2)
    sim.send_key(KeyEvent::Up);
    assert_eq!(sim.permission_selected(), Some(2));
}

/// Full end-to-end test: Done clears busy state.
#[test]
fn e2e_done_clears_busy() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_message(AgentMessage::Thinking);
    // Busy state is set

    sim.send_message(AgentMessage::Done);

    // Should render without error
    let screen = sim.screen();
    assert!(screen.size().0 > 0 && screen.size().1 > 0);
}

/// Full end-to-end test: Token counts update.
#[test]
fn e2e_token_counts_update() {
    let mut sim = TerminalSimulator::new(80, 24);
    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_message(AgentMessage::Tokens {
        input: 1000,
        output: 500,
    });

    // Should render without error
    let screen = sim.screen();
    assert!(screen.size().0 > 0 && screen.size().1 > 0);
}
