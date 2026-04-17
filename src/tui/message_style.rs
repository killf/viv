//! Message formatting helpers mirroring Claude Code's chat output style.
//!
//! - User messages:  orange `>` followed by user text
//! - Assistant:      `● ` Claude-orange bullet, continuation rows indented 2 spaces
//! - Errors:         `● ` + text, all in error red
//! - Welcome:        `● viv  ready`
use crate::terminal::style::theme;
use crate::tui::paragraph::{Line, Span};

/// Format a user-entered line for the history area.
pub fn format_user_message(text: &str) -> Line {
    Line::from_spans(vec![
        Span::styled("> ", theme::CLAUDE, false),
        Span::raw(text),
    ])
}

/// Format an assistant response as one or more display lines.
/// The first line is prefixed with the `●` bullet in Claude orange;
/// subsequent lines are indented by 2 spaces to align with the text column.
pub fn format_assistant_message(response: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut parts = response.split('\n');
    let first = parts.next().unwrap_or("");
    lines.push(Line::from_spans(vec![
        Span::styled("● ", theme::CLAUDE, false),
        Span::raw(first),
    ]));
    for rest in parts {
        lines.push(Line::from_spans(vec![
            Span::raw("  "),
            Span::raw(rest),
        ]));
    }
    lines
}

/// Format an error message (bullet + text, both in error red).
pub fn format_error_message(msg: &str) -> Vec<Line> {
    vec![Line::from_spans(vec![
        Span::styled("● ", theme::ERROR, false),
        Span::styled(msg.to_string(), theme::ERROR, false),
    ])]
}

/// The single-line startup banner.
pub fn format_welcome() -> Line {
    Line::from_spans(vec![
        Span::styled("● ", theme::CLAUDE, false),
        Span::styled("viv", theme::CLAUDE, true),
        Span::raw("  "),
        Span::styled("ready", theme::DIM, false),
    ])
}
