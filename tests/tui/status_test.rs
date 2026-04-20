use viv::core::terminal::buffer::{Buffer, Rect};
use viv::core::terminal::style::theme;
use viv::tui::status::StatusWidget;
use viv::tui::widget::Widget;

#[test]
fn renders_model_name() {
    let w = StatusWidget {
        cwd: "/home/user".to_string(),
        branch: Some("main".to_string()),
        model: "claude-sonnet-4-6".to_string(),
        input_tokens: 0,
        output_tokens: 0,
    };
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 1));
    w.render(Rect::new(0, 0, 60, 1), &mut buf);
    let rendered: String = (0..60).map(|x| buf.get(x, 0).ch).collect();
    assert!(
        rendered.contains("claude-sonnet-4-6"),
        "model name should appear"
    );
}

#[test]
fn renders_token_counts() {
    let w = StatusWidget {
        cwd: "/home/user".to_string(),
        branch: None,
        model: "m".to_string(),
        input_tokens: 1000,
        output_tokens: 250,
    };
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 1));
    w.render(Rect::new(0, 0, 60, 1), &mut buf);
    let rendered: String = (0..60).map(|x| buf.get(x, 0).ch).collect();
    assert!(rendered.contains("1000"), "input tokens");
    assert!(rendered.contains("250"), "output tokens");
}

#[test]
fn cost_calculation_sonnet_pricing() {
    let w = StatusWidget {
        cwd: "/home/user".to_string(),
        branch: Some("main".to_string()),
        model: "m".to_string(),
        input_tokens: 1_000_000,
        output_tokens: 1_000_000,
    };
    // Sonnet: $3/M input + $15/M output = $18 total
    let cost = w.estimate_cost();
    assert!(
        (cost - 18.0).abs() < 0.001,
        "expected $18.000, got {}",
        cost
    );
}

#[test]
fn zero_tokens_shows_zero_cost() {
    let w = StatusWidget {
        cwd: "/home/user".to_string(),
        branch: None,
        model: "m".to_string(),
        input_tokens: 0,
        output_tokens: 0,
    };
    assert_eq!(w.estimate_cost(), 0.0);
}

#[test]
fn text_is_dim() {
    let w = StatusWidget {
        cwd: "/home/user".to_string(),
        branch: Some("main".to_string()),
        model: "m".to_string(),
        input_tokens: 0,
        output_tokens: 0,
    };
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
    w.render(Rect::new(0, 0, 40, 1), &mut buf);
    // First non-space cell should be dim (col 2 due to 2 leading spaces)
    assert_eq!(buf.get(2, 0).fg, Some(theme::DIM));
}
