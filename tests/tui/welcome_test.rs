use viv::core::terminal::buffer::{Buffer, Rect};
use viv::core::terminal::style::Color;
use viv::tui::welcome::WelcomeWidget;
use viv::tui::widget::Widget;

/// Box layout: 2 border rows + 9 content rows = 11.
#[test]
fn welcome_height_is_eleven() {
    assert_eq!(WelcomeWidget::HEIGHT, 11);
}

/// Logo starts at content row 3 → absolute row 4 (border + 3 blank/welcome rows).
#[test]
fn welcome_renders_logo() {
    let widget = WelcomeWidget::new(Some("claude-sonnet-4-6"), "~/projects/viv");
    let area = Rect::new(0, 0, 80, 11);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // Row 4 contains logo line 1 (▐▛███▜▌ centered in left panel).
    let row4: String = (0..80).map(|x| buf.get(x, 4).ch).collect();
    assert!(
        row4.contains('▐'),
        "logo row 4 should contain '▐': got '{row4}'"
    );
}

/// Model is centered in the left panel at content row 7 → absolute row 8.
#[test]
fn welcome_renders_model_info() {
    let widget = WelcomeWidget::new(Some("claude-sonnet-4-6"), "~/projects/viv");
    let area = Rect::new(0, 0, 80, 11);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let row8: String = (0..80).map(|x| buf.get(x, 8).ch).collect();
    assert!(
        row8.contains("claude-sonnet-4-6"),
        "row 8 should contain model name: got '{row8}'"
    );
}

#[test]
fn welcome_renders_placeholder_when_no_model() {
    let widget = WelcomeWidget::new(None, "~/projects/viv");
    let area = Rect::new(0, 0, 80, 11);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let row8: String = (0..80).map(|x| buf.get(x, 8).ch).collect();
    assert!(
        row8.contains("..."),
        "row 8 should show '...' when model unknown: got '{row8}'"
    );
}

/// CWD is centered in the left panel at content row 8 → absolute row 9.
#[test]
fn welcome_renders_cwd_info() {
    let widget = WelcomeWidget::new(Some("test-model"), "~/my/path");
    let area = Rect::new(0, 0, 80, 11);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let row9: String = (0..80).map(|x| buf.get(x, 9).ch).collect();
    assert!(row9.contains("~/my/path"), "row 9 should show CWD: got '{row9}'");
}

/// Logo characters in the left panel (cols 1-36, rows 4-6) use CLAUDE orange.
#[test]
fn welcome_logo_uses_claude_color() {
    let widget = WelcomeWidget::new(Some("m"), "~");
    let area = Rect::new(0, 0, 80, 11);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // Find any non-space cell in the left panel of logo rows (rows 4-6, cols 1-36).
    let logo_cell = (4u16..7).flat_map(|y| (1u16..37).map(move |x| (x, y)))
        .map(|(x, y)| buf.get(x, y))
        .find(|c| c.ch != ' ');
    if let Some(cell) = logo_cell {
        assert_eq!(
            cell.fg,
            Some(Color::Rgb(215, 119, 87)),
            "logo should use CLAUDE orange"
        );
    }
}

/// Right panel row 0 shows "Tips for getting started".
#[test]
fn welcome_renders_tips_header() {
    let widget = WelcomeWidget::new(Some("m"), "~");
    let area = Rect::new(0, 0, 80, 11);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // cr=0 → absolute row 1.
    let row1: String = (0..80).map(|x| buf.get(x, 1).ch).collect();
    assert!(
        row1.contains("Tips for getting started"),
        "row 1 should show tips header: got '{row1}'"
    );
}

/// Right panel row 3 shows "Recent activity".
#[test]
fn welcome_renders_recent_activity() {
    let widget = WelcomeWidget::new(Some("m"), "~");
    let area = Rect::new(0, 0, 80, 11);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // cr=3 → absolute row 4.
    let row4: String = (0..80).map(|x| buf.get(x, 4).ch).collect();
    assert!(
        row4.contains("Recent activity"),
        "row 4 should show 'Recent activity': got '{row4}'"
    );
}
