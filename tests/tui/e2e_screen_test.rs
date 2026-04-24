//! End-to-end UI tests for SimTerminal.
//!
//! Drives the real TuiSession through SimTerminal and asserts the complete
//! rendered screen (text + truecolor foreground) produced by the production
//! rendering pipeline.

use viv::agent::protocol::AgentMessage;
use viv::core::terminal::simulator::SimTerminal;

// Box geometry (must match welcome.rs):
//   LEFT_WIDTH=36, RIGHT_WIDTH=41, CONTENT_ROWS=9, BOX_HEIGHT=11
// Terminal height 16 = box(11) + blank(1) + live(4)

/// Complete 80×16 welcome screen: box (rows 0-10), 1 blank (row 11), live region (rows 12-15).
#[test]
fn e2e_welcome_screen_layout() {
    let mut sim = SimTerminal::new(80, 16)
        .with_cwd("/data/project")
        .with_shell("zsh")
        .with_platform("linux x86_64");
    sim.send_message(AgentMessage::Ready {
        model: "claude-3-5-sonnet-20241022".into(),
    });

    let screen = sim.screen();

    screen.assert_screen(&[
        // Rows 0-10: welcome box (11 rows)
        "╭─── viv ──────────────────────────────────────────────────────────────────────╮",
        "│                                    │ Tips for getting started                │",
        "│           Welcome to viv!          │ Run /help to see available commands     │",
        "│                                    │ ─────────────────────────────────────── │",
        "│               ▐▛███▜▌              │ Recent activity                         │",
        "│              ▝▜█████▛▘             │ No recent activity                      │",
        "│                ▘▘ ▝▝               │                                         │",
        "│                                    │                                         │",
        "│     claude-3-5-sonnet-20241022     │                                         │",
        "│            /data/project           │                                         │",
        "╰──────────────────────────────────────────────────────────────────────────────╯",
        // Row 11: 1 blank between box and live region
        "",
        // Rows 12-15: live region (bottom-pinned)
        "────────────────────────────────────────────────────────────────────────────────",
        "\u{276F}",
        "────────────────────────────────────────────────────────────────────────────────",
        "  ? for shortcuts                                   ● claude-3-5-sonnet-20241022",
    ]);
}

/// Simulate pre-existing shell output (e.g. an `ls` before viv launched):
/// welcome box lands below that content; live region pins to the bottom.
/// Terminal height 18 = ls(2) + box(11) + blank(1) + live(4).
#[test]
fn e2e_welcome_after_simulated_command() {
    let mut sim = SimTerminal::new(80, 18)
        .with_cwd("/data/project")
        .with_shell("zsh")
        .with_platform("linux x86_64");

    sim.simulate_command("ls", "Cargo.toml  src  tests");
    sim.send_message(AgentMessage::Ready {
        model: "claude-3-5-sonnet-20241022".into(),
    });

    let screen = sim.screen();

    screen.assert_screen(&[
        // Rows 0-1: simulated `ls` output.
        "$ ls",
        "Cargo.toml  src  tests",
        // Rows 2-12: welcome box (11 rows).
        "╭─── viv ──────────────────────────────────────────────────────────────────────╮",
        "│                                    │ Tips for getting started                │",
        "│           Welcome to viv!          │ Run /help to see available commands     │",
        "│                                    │ ─────────────────────────────────────── │",
        "│               ▐▛███▜▌              │ Recent activity                         │",
        "│              ▝▜█████▛▘             │ No recent activity                      │",
        "│                ▘▘ ▝▝               │                                         │",
        "│                                    │                                         │",
        "│     claude-3-5-sonnet-20241022     │                                         │",
        "│            /data/project           │                                         │",
        "╰──────────────────────────────────────────────────────────────────────────────╯",
        // Row 13: 1 blank.
        "",
        // Rows 14-17: live region (bottom-pinned).
        "────────────────────────────────────────────────────────────────────────────────",
        "\u{276F}",
        "────────────────────────────────────────────────────────────────────────────────",
        "  ? for shortcuts                                   ● claude-3-5-sonnet-20241022",
    ]);
}

/// Growing the terminal from 24 to 30 rows must move the live region
/// down so it stays pinned to the new bottom.
#[test]
fn e2e_resize_repins_live_region_to_bottom() {
    let mut sim = SimTerminal::new(80, 24)
        .with_cwd("/data/project")
        .with_shell("zsh")
        .with_platform("linux x86_64");
    sim.send_message(AgentMessage::Ready {
        model: "claude-3-5-sonnet-20241022".into(),
    });

    // Pre-condition: welcome box at the top, live region pinned to rows 20-23.
    let before = sim.screen();
    assert_eq!(before.size(), (80, 24));
    before.assert_screen(&[
        // Rows 0-10: welcome box.
        "╭─── viv ──────────────────────────────────────────────────────────────────────╮",
        "│                                    │ Tips for getting started                │",
        "│           Welcome to viv!          │ Run /help to see available commands     │",
        "│                                    │ ─────────────────────────────────────── │",
        "│               ▐▛███▜▌              │ Recent activity                         │",
        "│              ▝▜█████▛▘             │ No recent activity                      │",
        "│                ▘▘ ▝▝               │                                         │",
        "│                                    │                                         │",
        "│     claude-3-5-sonnet-20241022     │                                         │",
        "│            /data/project           │                                         │",
        "╰──────────────────────────────────────────────────────────────────────────────╯",
        // Rows 11-19: blank (9 rows in a 24-row terminal).
        "", "", "", "", "", "", "", "", "",
        // Rows 20-23: live region.
        "────────────────────────────────────────────────────────────────────────────────",
        "\u{276F}",
        "────────────────────────────────────────────────────────────────────────────────",
        "  ? for shortcuts                                   ● claude-3-5-sonnet-20241022",
    ]);

    // Grow the terminal height from 24 to 30 rows.
    sim.resize(80, 30);

    // Post-condition: live region moves to the new bottom (rows 26-29).
    let after = sim.screen();
    assert_eq!(after.size(), (80, 30));
    after.assert_screen(&[
        "", "", "", "", "", "", "", "", "", "",
        "", "", "", "", "", "", "", "", "", "",
        "", "", "", "", "", "",
        "────────────────────────────────────────────────────────────────────────────────",
        "\u{276F}",
        "────────────────────────────────────────────────────────────────────────────────",
        "  ? for shortcuts                                   ● claude-3-5-sonnet-20241022",
    ]);
}
