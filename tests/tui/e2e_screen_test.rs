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
    let mut sim = SimTerminal::new(80, 24)
        .with_cwd("/data/project")
        .with_shell("zsh")
        .with_platform("linux x86_64");
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

/// Simulate pre-existing shell output (e.g. an `ls` before viv launched):
/// welcome should land below that content and the status bar still pins
/// to the bottom row.
#[test]
fn e2e_welcome_after_simulated_command() {
    let mut sim = SimTerminal::new(80, 24)
        .with_cwd("/data/project")
        .with_shell("zsh")
        .with_platform("linux x86_64");

    sim.simulate_command("ls", "Cargo.toml  src  tests");
    sim.send_message(AgentMessage::Ready {
        model: "claude-3-5-sonnet-20241022".into(),
    });

    let screen = sim.screen();

    screen.assert_screen(&[
        // Rows 0-1: simulated `ls` output (prompt + files).
        "$ ls",
        "Cargo.toml  src  tests",
        // Rows 2-6: welcome, starting right below the ls output.
        r"       _           Model:    claude-3-5-sonnet-20241022",
        r"__   _(_)_   __    CWD:      /data/project",
        r"\ \ / / \ \ / /    Branch:   -",
        r" \ V /| |\ V /     Platform: linux x86_64",
        r"  \_/ |_| \_/      Shell:    zsh",
        // Rows 7-19: blank (welcome trailing blanks + gap before live region).
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
        // Rows 20-23: live region (bottom-pinned, unchanged by scrollback above).
        "────────────────────────────────────────────────────────────────────────────────",
        "\u{276F}",
        "────────────────────────────────────────────────────────────────────────────────",
        "  /data/project                    claude-3-5-sonnet-20241022  \u{2191} 0  \u{2193} 0  ~$0.000",
    ]);
}

/// Growing the terminal from 24 to 30 rows must move the live region
/// down so it stays pinned to the new bottom. SimTerminal::resize
/// rebuilds the parser, then the session redraws the live region via
/// absolute positioning; upper rows are left blank in the fresh screen.
#[test]
fn e2e_resize_repins_live_region_to_bottom() {
    let mut sim = SimTerminal::new(80, 24)
        .with_cwd("/data/project")
        .with_shell("zsh")
        .with_platform("linux x86_64");
    sim.send_message(AgentMessage::Ready {
        model: "claude-3-5-sonnet-20241022".into(),
    });

    // Pre-condition: welcome at the top, live region pinned to the bottom
    // of a 24-row screen (rows 20-23).
    let before = sim.screen();
    assert_eq!(before.size(), (80, 24));
    before.assert_screen(&[
        r"       _           Model:    claude-3-5-sonnet-20241022",
        r"__   _(_)_   __    CWD:      /data/project",
        r"\ \ / / \ \ / /    Branch:   -",
        r" \ V /| |\ V /     Platform: linux x86_64",
        r"  \_/ |_| \_/      Shell:    zsh",
        "", "", "", "", "", "", "", "", "", "", "", "", "", "", "",
        "────────────────────────────────────────────────────────────────────────────────",
        "\u{276F}",
        "────────────────────────────────────────────────────────────────────────────────",
        "  /data/project                    claude-3-5-sonnet-20241022  \u{2191} 0  \u{2193} 0  ~$0.000",
    ]);

    // Grow the terminal height from 24 to 30 rows.
    sim.resize(80, 30);

    // Post-condition: live region moves to the new bottom (rows 26-29).
    // Welcome is not re-emitted, so the upper rows are blank.
    let after = sim.screen();
    assert_eq!(after.size(), (80, 30));
    after.assert_screen(&[
        "", "", "", "", "", "", "", "", "", "",
        "", "", "", "", "", "", "", "", "", "",
        "", "", "", "", "", "",
        "────────────────────────────────────────────────────────────────────────────────",
        "\u{276F}",
        "────────────────────────────────────────────────────────────────────────────────",
        "  /data/project                    claude-3-5-sonnet-20241022  \u{2191} 0  \u{2193} 0  ~$0.000",
    ]);
}

