use crate::core::terminal::style::theme;
use crate::tui::paragraph::{Line, Span};

/// Render a permission-pending prompt line.
///
/// Renders: `  ◆ {tool}({summary}) [y/n]`
/// - `◆` bullet in `theme::SUGGESTION`
/// - tool name in default color (white)
/// - `({summary})` and ` [y/n]` in `theme::DIM`
pub fn render_permission_pending(tool: &str, summary: &str) -> Line {
    Line::from_spans(vec![
        Span::styled("  \u{25c6} ", theme::SUGGESTION, false),
        Span::raw(tool),
        Span::styled(format!("({})", summary), theme::DIM, false),
        Span::styled(" [y/n]", theme::DIM, false),
    ])
}

/// Render a permission-result line (after the user has responded).
///
/// If allowed: `  ✓ Allowed  {tool} ({summary})` with success color
/// If denied:  `  ✗ Denied   {tool} ({summary})` with error color
pub fn render_permission_result(tool: &str, summary: &str, allowed: bool) -> Line {
    if allowed {
        Line::from_spans(vec![
            Span::styled("  \u{2713} ", theme::SUCCESS, false),
            Span::styled("Allowed", theme::SUCCESS, false),
            Span::styled(format!("  {} ({})", tool, summary), theme::DIM, false),
        ])
    } else {
        Line::from_spans(vec![
            Span::styled("  \u{2717} ", theme::ERROR, false),
            Span::styled("Denied", theme::ERROR, false),
            Span::styled(format!("  {} ({})", tool, summary), theme::DIM, false),
        ])
    }
}
