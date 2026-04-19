use viv::core::terminal::buffer::{Buffer, Rect};
use viv::core::terminal::style::Color;
use viv::tui::welcome::WelcomeWidget;
use viv::tui::widget::Widget;

#[test]
fn welcome_height_is_five() {
    assert_eq!(WelcomeWidget::HEIGHT, 5);
}

#[test]
fn welcome_renders_logo() {
    let widget = WelcomeWidget::new(
        Some("claude-sonnet-4-6"),
        "~/projects/viv",
        Some("main"),
    );
    let area = Rect::new(0, 0, 60, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let row1: String = (0..20).map(|x| buf.get(x, 1).ch).collect();
    assert!(row1.contains('_'), "logo row 1 should contain '_': got '{row1}'");
}

#[test]
fn welcome_renders_model_info() {
    let widget = WelcomeWidget::new(
        Some("claude-sonnet-4-6"),
        "~/projects/viv",
        Some("main"),
    );
    let area = Rect::new(0, 0, 60, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let full: String = (0..60).map(|x| buf.get(x, 0).ch).collect();
    assert!(full.contains("Model"), "should contain Model label: got '{full}'");
}

#[test]
fn welcome_renders_placeholder_when_no_model() {
    let widget = WelcomeWidget::new(
        None,
        "~/projects/viv",
        Some("main"),
    );
    let area = Rect::new(0, 0, 60, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let full: String = (0..60).map(|x| buf.get(x, 0).ch).collect();
    assert!(full.contains("..."), "should show '...' when model unknown: got '{full}'");
}

#[test]
fn welcome_renders_cwd_info() {
    let widget = WelcomeWidget::new(
        Some("test-model"),
        "~/my/path",
        None,
    );
    let area = Rect::new(0, 0, 60, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let row1: String = (0..60).map(|x| buf.get(x, 1).ch).collect();
    assert!(row1.contains("~/my/path"), "should show CWD: got '{row1}'");
}

#[test]
fn welcome_logo_uses_claude_color() {
    let widget = WelcomeWidget::new(Some("m"), "~", None);
    let area = Rect::new(0, 0, 60, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let logo_cell = (0..20).map(|x| buf.get(x, 1))
        .find(|c| c.ch != ' ');
    if let Some(cell) = logo_cell {
        assert_eq!(cell.fg, Some(Color::Rgb(215, 119, 87)), "logo should use CLAUDE orange");
    }
}
