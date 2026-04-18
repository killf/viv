use viv::core::terminal::buffer::{Buffer, Rect};
use viv::core::terminal::style::theme;
use viv::tui::header::HeaderWidget;
use viv::tui::widget::Widget;

#[test]
fn renders_cwd_without_branch() {
    let w = HeaderWidget {
        cwd: "~/project".to_string(),
        branch: None,
    };
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
    w.render(Rect::new(0, 0, 40, 1), &mut buf);
    // Check '~' appears at col 2 (two leading spaces)
    assert_eq!(buf.get(2, 0).ch, '~');
}

#[test]
fn renders_branch_when_present() {
    let w = HeaderWidget {
        cwd: "~/p".to_string(),
        branch: Some("main".to_string()),
    };
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
    w.render(Rect::new(0, 0, 40, 1), &mut buf);
    // Text should contain ⎇
    let rendered: String = (0..40).map(|x| buf.get(x, 0).ch).collect();
    assert!(rendered.contains('⎇'), "should contain branch symbol");
}

#[test]
fn truncates_long_cwd() {
    let long = "~/very/long/path/that/exceeds/thirty/chars/yes";
    let w = HeaderWidget::from_path(long, None);
    assert!(w.cwd.chars().count() <= 30); // "…" + 29 chars
}

#[test]
fn text_is_dim() {
    let w = HeaderWidget {
        cwd: "~/p".to_string(),
        branch: None,
    };
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
    w.render(Rect::new(0, 0, 20, 1), &mut buf);
    assert_eq!(buf.get(2, 0).fg, Some(theme::DIM));
}

#[test]
fn parse_git_branch_from_head_content() {
    let content = "ref: refs/heads/my-feature\n";
    assert_eq!(
        viv::tui::header::parse_branch(content),
        Some("my-feature".to_string())
    );
}

#[test]
fn parse_git_branch_detached_head_returns_none() {
    let content = "abc1234567890abcdef1234567890abcdef12345\n";
    assert_eq!(viv::tui::header::parse_branch(content), None);
}
