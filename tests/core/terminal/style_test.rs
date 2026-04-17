use viv::core::terminal::style::{Color, theme};

#[test]
fn color_ansi_foreground_bytes() {
    assert_eq!(Color::Ansi(31).fg_seq(), "\x1b[31m");
    assert_eq!(Color::Ansi(36).fg_seq(), "\x1b[36m");
    assert_eq!(Color::Ansi(90).fg_seq(), "\x1b[90m");
}

#[test]
fn color_rgb_foreground_bytes() {
    assert_eq!(Color::Rgb(215, 119, 87).fg_seq(), "\x1b[38;2;215;119;87m");
    assert_eq!(Color::Rgb(0, 0, 0).fg_seq(), "\x1b[38;2;0;0;0m");
    assert_eq!(Color::Rgb(255, 255, 255).fg_seq(), "\x1b[38;2;255;255;255m");
}

#[test]
fn color_ansi_background_bytes() {
    // bg = fg + 10
    assert_eq!(Color::Ansi(31).bg_seq(), "\x1b[41m");
    assert_eq!(Color::Ansi(36).bg_seq(), "\x1b[46m");
}

#[test]
fn color_rgb_background_bytes() {
    assert_eq!(Color::Rgb(50, 50, 50).bg_seq(), "\x1b[48;2;50;50;50m");
}

#[test]
fn color_equality() {
    assert_eq!(Color::Rgb(1, 2, 3), Color::Rgb(1, 2, 3));
    assert_ne!(Color::Rgb(1, 2, 3), Color::Rgb(3, 2, 1));
    assert_ne!(Color::Ansi(31), Color::Rgb(255, 0, 0));
}

#[test]
fn theme_has_claude_orange() {
    let c = theme::CLAUDE;
    assert_eq!(c, Color::Rgb(215, 119, 87));
}

#[test]
fn theme_has_dim_gray() {
    let c = theme::DIM;
    assert_eq!(c, Color::Rgb(136, 136, 136));
}

#[test]
fn theme_has_suggestion_blue() {
    let c = theme::SUGGESTION;
    assert_eq!(c, Color::Rgb(177, 185, 249));
}

#[test]
fn theme_has_success_green() {
    let c = theme::SUCCESS;
    assert_eq!(c, Color::Rgb(78, 186, 101));
}

#[test]
fn theme_has_error_red() {
    let c = theme::ERROR;
    assert_eq!(c, Color::Rgb(171, 43, 63));
}

#[test]
fn theme_has_text_white() {
    let c = theme::TEXT;
    assert_eq!(c, Color::Rgb(255, 255, 255));
}
