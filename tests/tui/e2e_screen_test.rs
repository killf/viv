//! End-to-end UI tests for TerminalSimulator.
//!
//! These tests assert the COMPLETE screen layout using assert_screen().

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
