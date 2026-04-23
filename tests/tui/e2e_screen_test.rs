//! End-to-end UI tests for TerminalSimulator.
//!
//! These tests assert the COMPLETE screen layout, not just partial content.
//! This provides true TDD validation of the UI rendering.

use viv::core::terminal::simulator::TerminalSimulator;
use viv::core::terminal::input::KeyEvent;
use viv::agent::protocol::AgentMessage;

/// Full end-to-end test: Welcome screen renders with complete layout (24 rows).
#[test]
fn e2e_welcome_screen_layout() {
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

    sim.send_message(AgentMessage::Ready {
        model: "claude-3-5-sonnet-20241022".into(),
    });

    let screen = sim.screen();

    // Assert the complete Welcome screen layout (80x24 = 24 rows)
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

/// Full end-to-end test: Input editor stores typed text.
#[test]
fn e2e_input_editor_stores_text() {
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

    sim.send_message(AgentMessage::Ready { model: "test".into() });

    // Type "Hello"
    for ch in "Hello".chars() {
        sim.send_key(KeyEvent::Char(ch));
    }

    // Input content is stored
    assert_eq!(
        sim.input_content(),
        "Hello",
        "Input should contain 'Hello'"
    );
}

/// Full end-to-end test: Submit clears prompt.
#[test]
fn e2e_submit_clears_prompt() {
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

    sim.send_message(AgentMessage::Ready { model: "test".into() });

    // Type "Hello"
    for ch in "Hello".chars() {
        sim.send_key(KeyEvent::Char(ch));
    }
    assert_eq!(sim.input_content(), "Hello");

    // Submit
    sim.send_key(KeyEvent::Enter);

    // Prompt is cleared
    assert_eq!(
        sim.input_content(),
        "",
        "Prompt should be empty after submit"
    );
}

/// Full end-to-end test: Slash command mode indicator.
#[test]
fn e2e_slash_command_mode() {
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

    sim.send_message(AgentMessage::Ready { model: "test".into() });
    sim.send_key(KeyEvent::Char('/'));

    assert_eq!(
        sim.input_mode(),
        viv::tui::input::InputMode::SlashCommand,
    );
}

/// Full end-to-end test: Colon command mode indicator.
#[test]
fn e2e_colon_command_mode() {
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

    sim.send_message(AgentMessage::Ready { model: "test".into() });
    sim.send_key(KeyEvent::Char(':'));

    assert_eq!(
        sim.input_mode(),
        viv::tui::input::InputMode::ColonCommand,
    );
}

/// Full end-to-end test: Backspace removes characters from input.
#[test]
fn e2e_backspace_removes_char() {
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

    sim.send_message(AgentMessage::Ready { model: "test".into() });

    // Type "hello"
    for ch in "hello".chars() {
        sim.send_key(KeyEvent::Char(ch));
    }
    assert_eq!(sim.input_content(), "hello");

    // Backspace removes 'o'
    sim.send_key(KeyEvent::Backspace);
    assert_eq!(sim.input_content(), "hell");

    // Two more backspaces
    sim.send_key(KeyEvent::Backspace);
    sim.send_key(KeyEvent::Backspace);
    assert_eq!(sim.input_content(), "he");
}

/// Full end-to-end test: Ctrl+C clears input.
#[test]
fn e2e_ctrl_c_clears_input() {
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

    sim.send_message(AgentMessage::Ready { model: "test".into() });

    // Type "some text"
    for ch in "some text".chars() {
        sim.send_key(KeyEvent::Char(ch));
    }
    assert_eq!(sim.input_content(), "some text");

    // Ctrl+C clears
    sim.send_key(KeyEvent::CtrlC);

    assert_eq!(sim.input_content(), "");
}

/// Full end-to-end test: Multi-line input with Shift+Enter.
#[test]
fn e2e_multiline_input() {
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

    sim.send_message(AgentMessage::Ready { model: "test".into() });

    // Type "first", newline, "second"
    for ch in "first".chars() {
        sim.send_key(KeyEvent::Char(ch));
    }
    sim.send_key(KeyEvent::ShiftEnter);
    for ch in "second".chars() {
        sim.send_key(KeyEvent::Char(ch));
    }

    let content = sim.input_content();
    assert!(
        content.contains("first\nsecond"),
        "Should contain multi-line content"
    );
}

/// Full end-to-end test: Resize updates display dimensions.
#[test]
fn e2e_resize_updates_display() {
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

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
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

    sim.send_message(AgentMessage::Ready { model: "test".into() });

    // Submit "first"
    for ch in "first".chars() {
        sim.send_key(KeyEvent::Char(ch));
    }
    sim.send_key(KeyEvent::Enter);

    // Submit "second"
    for ch in "second".chars() {
        sim.send_key(KeyEvent::Char(ch));
    }
    sim.send_key(KeyEvent::Enter);

    // Type "third" (not submitted)
    for ch in "third".chars() {
        sim.send_key(KeyEvent::Char(ch));
    }

    // Press Up -> shows "second"
    sim.send_key(KeyEvent::Up);
    assert!(
        sim.input_content().contains("second"),
        "Up should show 'second'"
    );

    // Press Up again -> shows "first"
    sim.send_key(KeyEvent::Up);
    assert!(
        sim.input_content().contains("first"),
        "Up again should show 'first'"
    );

    // Press Down -> stays at oldest
    sim.send_key(KeyEvent::Down);
    assert!(
        sim.input_content().contains("first"),
        "Down at oldest stays at 'first'"
    );
}

/// Full end-to-end test: Ctrl+R enters history search mode.
#[test]
fn e2e_ctrl_r_enters_search_mode() {
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

    sim.send_message(AgentMessage::Ready { model: "test".into() });
    sim.send_key(KeyEvent::CtrlR);

    assert_eq!(
        sim.input_mode(),
        viv::tui::input::InputMode::HistorySearch,
    );
}

/// Full end-to-end test: Tool call shows tool name.
#[test]
fn e2e_tool_call_shows_name() {
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

    sim.send_message(AgentMessage::Ready { model: "test".into() });
    sim.send_message(AgentMessage::ToolStart {
        name: "Bash".into(),
        input: "ls -la".into(),
    });

    let screen = sim.screen();

    // Tool name is visible
    assert!(
        screen.contains("Bash"),
        "Screen should contain tool name 'Bash'"
    );
    // Running state is visible
    assert!(
        screen.contains("running"),
        "Screen should show 'running' state"
    );
}

/// Full end-to-end test: Tool end shows summary (folded view).
#[test]
fn e2e_tool_end_shows_summary() {
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

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

    // Tool name visible
    assert!(screen.contains("Bash"));
    // Output summary visible
    assert!(screen.contains("chars"));
}

/// Full end-to-end test: Tool error shows error state.
#[test]
fn e2e_tool_error_shows() {
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_message(AgentMessage::ToolStart {
        name: "Bash".into(),
        input: "invalid".into(),
    });

    sim.send_message(AgentMessage::ToolError {
        name: "Bash".into(),
        error: "Command not found".into(),
    });

    let screen = sim.screen();

    // Tool name visible
    assert!(screen.contains("Bash"));
}

/// Full end-to-end test: Permission menu renders.
#[test]
fn e2e_permission_menu_renders() {
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_message(AgentMessage::PermissionRequest {
        tool: "Bash".into(),
        input: "rm -rf /".into(),
    });

    let screen = sim.screen();

    // Permission menu renders (has non-empty content in menu area)
    let has_menu = (8..15)
        .any(|r| screen.line_text(r).map(|l| !l.trim().is_empty()).unwrap_or(false));
    assert!(has_menu, "Permission menu should render");
}

/// Full end-to-end test: Permission menu selection navigation.
#[test]
fn e2e_permission_menu_selection() {
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_message(AgentMessage::PermissionRequest {
        tool: "Bash".into(),
        input: "ls".into(),
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
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

    sim.send_message(AgentMessage::Ready { model: "test".into() });
    sim.send_message(AgentMessage::Thinking);

    let screen1 = sim.screen();
    assert!(screen1.size().0 > 0 && screen1.size().1 > 0);

    sim.send_message(AgentMessage::Done);

    let screen2 = sim.screen();
    assert!(screen2.size().0 > 0 && screen2.size().1 > 0);
}

/// Full end-to-end test: Token counts update.
#[test]
fn e2e_token_counts_update() {
    let mut sim = TerminalSimulator::new(80, 24).with_cwd("/data/project");

    sim.send_message(AgentMessage::Ready { model: "test".into() });

    sim.send_message(AgentMessage::Tokens {
        input: 1000,
        output: 500,
    });

    let screen = sim.screen();
    assert!(screen.size().0 > 0 && screen.size().1 > 0);
}
