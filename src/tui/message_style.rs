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
    let mut rendered = crate::tui::markdown::render_markdown(response);
    let mut lines = Vec::new();
    let mut iter = rendered.drain(..);
    if let Some(first) = iter.next() {
        let mut first_spans = vec![Span::styled("● ", theme::CLAUDE, false)];
        first_spans.extend(first.spans);
        lines.push(Line::from_spans(first_spans));
        for line in iter {
            let mut prefix_spans = vec![Span::raw("  ")];
            prefix_spans.extend(line.spans);
            lines.push(Line::from_spans(prefix_spans));
        }
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
