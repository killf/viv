//! End-to-end UI tests for SimTerminal.
//!
//! Drives the real TuiSession through SimTerminal and asserts the complete
//! rendered screen (text + truecolor foreground) produced by the production
//! rendering pipeline.

use viv::agent::protocol::AgentMessage;
use viv::core::terminal::simulator::SimTerminal;

/// Complete 80x24 Welcome screen: logo, info rows, input frame, status bar.
#[test]
fn e2e_welcome_screen_layout() {
    // Shell appears in the welcome header; pin it so the test is reproducible.
    // Safety: single-threaded test; no other threads read SHELL here.
    unsafe { std::env::set_var("SHELL", "/bin/zsh"); }

    let mut sim = SimTerminal::new(80, 24).with_cwd("/data/project");
    sim.send_message(AgentMessage::Ready {
        model: "claude-3-5-sonnet-20241022".into(),
    });

    let screen = sim.screen();

    screen.assert_screen(&[
        r"       _           Model:    claude-3-5-sonnet-20241022",
        r"__   _(_)_   __    CWD:      /data/project",
        r"\ \ / / \ \ / /    Branch:   -",
        r" \ V /| |\ V /     Platform: linux x86_64",
        r"  \_/ |_| \_/      Shell:    zsh",
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
        "────────────────────────────────────────────────────────────────────────────────",
        "\u{276F}",
        "────────────────────────────────────────────────────────────────────────────────",
        "  /data/project                    claude-3-5-sonnet-20241022  \u{2191} 0  \u{2193} 0  ~$0.000",
    ]);

    // Logo uses CLAUDE orange, RGB(215, 119, 87).
    screen.assert_cell_fg_rgb(0, 7, 215, 119, 87);
    screen.assert_cell_fg_rgb(2, 0, 215, 119, 87);

    // Info labels use CLAUDE orange too.
    screen.assert_cell_fg_rgb(0, 19, 215, 119, 87);

    // Info values use TEXT white, RGB(255, 255, 255).
    screen.assert_cell_fg_rgb(0, 29, 255, 255, 255);
    screen.assert_cell_fg_rgb(1, 29, 255, 255, 255);

    // Input box border uses DIM, RGB(136, 136, 136).
    screen.assert_cell_fg_rgb(20, 0, 136, 136, 136);
    screen.assert_cell_fg_rgb(22, 79, 136, 136, 136);

    // Prompt glyph uses CLAUDE orange.
    screen.assert_cell_fg_rgb(21, 0, 215, 119, 87);

    // Status bar text (cwd + model) uses DIM.
    screen.assert_cell_fg_rgb(23, 2, 136, 136, 136);
    screen.assert_cell_fg_rgb(23, 35, 136, 136, 136);
}
