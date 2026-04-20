use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::style::Color;
use crate::tui::block::{Block, BorderStyle};
use crate::tui::paragraph::{Line, Span};
use crate::tui::widget::{StatefulWidget, Widget};

// ── Theme colors ─────────────────────────────────────────────────────────────

const SELECTED_BG: Color = Color::Rgb(177, 185, 249); // suggestion / periwinkle
const UNSELECTED_TEXT: Color = Color::Rgb(136, 136, 136); // DIM gray
const SELECTED_TEXT: Color = Color::Rgb(255, 255, 255); // white
const BORDER_COLOR: Color = Color::Rgb(80, 80, 80); // dark gray
const SELECTION_CHAR: &str = "\u{276F}"; // ▶ right-pointing triangle

// ── PermissionOption ─────────────────────────────────────────────────────────

/// One of the three permission choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionOption {
    Deny,
    Allow,
    AlwaysAllow,
}

impl PermissionOption {
    /// Chinese label for this option.
    pub fn label(&self) -> &'static str {
        match self {
            PermissionOption::Deny => "拒绝",
            PermissionOption::Allow => "允许",
            PermissionOption::AlwaysAllow => "总是允许",
        }
    }

    /// Short ASCII label (used for the result line).
    pub fn short_label(&self) -> &'static str {
        match self {
            PermissionOption::Deny => "Denied",
            PermissionOption::Allow => "Allowed",
            PermissionOption::AlwaysAllow => "AlwaysAllowed",
        }
    }
}

const ALL_OPTIONS: [PermissionOption; 3] = [
    PermissionOption::Deny,
    PermissionOption::Allow,
    PermissionOption::AlwaysAllow,
];

/// Returns the options in menu order.
pub const fn permission_options() -> &'static [PermissionOption] {
    &ALL_OPTIONS
}

// ── PermissionState ───────────────────────────────────────────────────────────

/// Mutable state for the permission menu widget.
#[derive(Debug, Clone)]
pub struct PermissionState {
    /// Index into [`permission_options`] that is currently selected.
    pub selected: usize,
}

impl PermissionState {
    pub fn new() -> Self {
        PermissionState { selected: 1 } // default: Allow (middle option)
    }

    /// Move selection up (wraps around).
    pub fn move_up(&mut self) {
        self.selected = if self.selected == 0 {
            permission_options().len() - 1
        } else {
            self.selected - 1
        };
    }

    /// Move selection down (wraps around).
    pub fn move_down(&mut self) {
        self.selected = (self.selected + 1) % permission_options().len();
    }

    /// Return the currently selected option.
    pub fn selected_option(&self) -> PermissionOption {
        permission_options()[self.selected]
    }
}

impl Default for PermissionState {
    fn default() -> Self {
        Self::new()
    }
}

// ── PermissionWidget ─────────────────────────────────────────────────────────

/// A widget that renders the permission options menu.
///
/// Renders inside the input area when a permission is pending:
///
/// ```text
///   ◆ tool_name(args)
///
///   ┌─────────────────────┐
///   │ ✗ 拒绝              │
///   │ > 允许              │
///   │  ◯ 总是允许          │
///   └─────────────────────┘
/// ```
#[allow(dead_code)]
pub struct PermissionWidget<'a> {
    tool: &'a str,
    input: &'a str,
}

impl<'a> PermissionWidget<'a> {
    /// Create a new permission widget.
    pub fn new(tool: &'a str, input: &'a str) -> Self {
        PermissionWidget { tool, input }
    }

    /// Returns the number of rows needed to render the options box.
    pub const fn height() -> u16 {
        // 3 option rows + top border + bottom border = 5
        5
    }

    /// Render a single option row into `buf` at (x, y).
    fn render_option(
        &self,
        x: u16,
        y: u16,
        option: PermissionOption,
        selected: bool,
        max_width: u16,
        buf: &mut Buffer,
    ) {
        let label = option.label();
        let icon = match option {
            PermissionOption::Deny => "\u{2717}", // ✗
            PermissionOption::Allow => "\u{25CB}", // ○ (unselected) / filled later
            PermissionOption::AlwaysAllow => "\u{25CB}",
        };

        // Compute display width of icon + space + label
        let icon_w = crate::core::terminal::buffer::char_width(icon.chars().next().unwrap());
        let space_w = 1u16;
        let label_w: u16 = label.chars().map(|c| crate::core::terminal::buffer::char_width(c)).sum();

        let total_w = icon_w + space_w + label_w;
        let indent = if max_width > total_w { (max_width - total_w) / 2 } else { 0 };
        let start_x = x + indent;

        // Background
        if selected {
            for col in x..(x + max_width) {
                let cell = buf.get_mut(col, y);
                cell.bg = Some(SELECTED_BG);
            }
        }

        // Selection indicator: "▶ " or "  "
        let sel_str = if selected { SELECTION_CHAR } else { " " };
        let sel_len = if selected {
            crate::core::terminal::buffer::char_width(SELECTION_CHAR.chars().next().unwrap())
        } else {
            0
        };
        let sel_x = start_x;
        if selected {
            buf.set_str(sel_x, y, sel_str, Some(SELECTED_TEXT), false);
        }

        // Icon
        let icon_color = match option {
            PermissionOption::Deny => Color::Rgb(171, 43, 63), // ERROR red
            PermissionOption::Allow => Color::Rgb(78, 186, 101), // SUCCESS green
            PermissionOption::AlwaysAllow => if selected {
                SELECTED_TEXT
            } else {
                UNSELECTED_TEXT
            },
        };
        let icon_x = sel_x + sel_len + 1;
        buf.set_str(icon_x, y, icon, Some(icon_color), false);

        // Label
        let label_x = icon_x + icon_w + 1;
        let label_color = if selected { SELECTED_TEXT } else { UNSELECTED_TEXT };
        buf.set_str(label_x, y, label, Some(label_color), false);
    }
}

impl StatefulWidget for PermissionWidget<'_> {
    type State = PermissionState;

    fn render(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() || area.height < Self::height() {
            return;
        }

        let options = permission_options();

        // ── Box border ─────────────────────────────────────────────────────────
        let block = Block::new()
            .border(BorderStyle::Rounded)
            .border_fg(BORDER_COLOR);
        block.render(area, buf);
        let inner = block.inner(area);

        if inner.is_empty() {
            return;
        }

        // ── Option rows (centered vertically) ──────────────────────────────────
        let total_rows = options.len() as u16;
        let start_y = inner.y + (inner.height.saturating_sub(total_rows)) / 2;

        for (i, &option) in options.iter().enumerate() {
            let y = start_y + i as u16;
            if y >= area.y + area.height {
                break;
            }
            let selected = i == state.selected;
            self.render_option(inner.x, y, option, selected, inner.width, buf);
        }
    }
}

// ── Legacy helpers (kept for tests / other callers) ───────────────────────────

/// Render a permission-pending prompt line.
///
/// Renders: `  ◆ {tool}({summary}) [y/n]`
/// - `◆` bullet in `theme::SUGGESTION`
/// - tool name in default color (white)
/// - `({summary})` and ` [y/n]` in `theme::DIM`
pub fn render_permission_pending(tool: &str, summary: &str) -> Line {
    use crate::core::terminal::style::theme;
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
    use crate::core::terminal::style::theme;
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
