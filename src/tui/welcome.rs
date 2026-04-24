use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::style::theme;
use crate::tui::widget::Widget;

const LOGO: [&str; 3] = [
    "▐▛███▜▌",
    "▝▜█████▛▘",
    "▘▘ ▝▝",
];

const LEFT_WIDTH: u16 = 36;
const RIGHT_WIDTH: u16 = 41;
const CONTENT_ROWS: u16 = 9;
// Minimum terminal width: │ + left + │ + right + │ = 80
const MIN_WIDTH: u16 = LEFT_WIDTH + RIGHT_WIDTH + 3;

pub struct WelcomeWidget<'a> {
    model: Option<&'a str>,
    cwd: &'a str,
}

impl<'a> WelcomeWidget<'a> {
    /// Total rendered height: 2 border rows + 9 content rows.
    pub const HEIGHT: u16 = CONTENT_ROWS + 2;

    pub const ROW_DELAY_MS: u64 = 80;
    pub const FADE_DURATION_MS: u64 = 200;
    pub const INFO_ROWS: usize = 0;
    pub const TOTAL_ROWS: u16 = Self::HEIGHT;

    pub fn new(model: Option<&'a str>, cwd: &'a str) -> Self {
        WelcomeWidget { model, cwd }
    }

    pub fn render_with_alpha(&self, area: Rect, buf: &mut Buffer, _info_alphas: &[f64]) {
        if area.is_empty() || area.height < Self::HEIGHT || area.width < MIN_WIDTH {
            return;
        }

        let x = area.x;
        let y = area.y;

        // Top border: ╭─── viv ──...──╮
        let dashes = (area.width as usize).saturating_sub(10);
        let top = format!("╭─── viv {}╮", "─".repeat(dashes));
        buf.set_str(x, y, &top, Some(theme::DIM), false);

        for cr in 0..CONTENT_ROWS {
            let ry = y + 1 + cr;

            buf.set_str(x, ry, "│", Some(theme::DIM), false);

            let left = self.left_content(cr);
            let left_fg = if (3..=5).contains(&cr) { theme::CLAUDE } else { theme::TEXT };
            buf.set_str(x + 1, ry, &left, Some(left_fg), false);

            buf.set_str(x + 1 + LEFT_WIDTH, ry, "│", Some(theme::DIM), false);

            if let Some(right) = self.right_content(cr) {
                buf.set_str(x + 1 + LEFT_WIDTH + 1, ry, &right, Some(theme::TEXT), false);
            }

            buf.set_str(x + 1 + LEFT_WIDTH + 1 + RIGHT_WIDTH, ry, "│", Some(theme::DIM), false);
        }

        // Bottom border: ╰──...──╯
        let bottom = format!("╰{}╯", "─".repeat(area.width as usize - 2));
        buf.set_str(x, y + 1 + CONTENT_ROWS, &bottom, Some(theme::DIM), false);
    }

    fn left_content(&self, cr: u16) -> String {
        let w = LEFT_WIDTH as usize;
        match cr {
            1 => center_pad("Welcome to viv!", w),
            3 => center_pad(LOGO[0], w),
            4 => center_pad(LOGO[1], w),
            5 => center_pad(LOGO[2], w),
            7 => center_pad(self.model.unwrap_or("..."), w),
            8 => center_pad(self.cwd, w),
            _ => " ".repeat(w),
        }
    }

    fn right_content(&self, cr: u16) -> Option<String> {
        let rw = RIGHT_WIDTH as usize;
        let lpad = |s: &str| format!("{:<rw$}", s, rw = rw);
        match cr {
            0 => Some(lpad(" Tips for getting started")),
            1 => Some(lpad(" Run /help to see available commands")),
            2 => Some(lpad(&format!(" {}", "─".repeat(rw - 2)))),
            3 => Some(lpad(" Recent activity")),
            4 => Some(lpad(" No recent activity")),
            _ => None,
        }
    }

    /// Serialize into ANSI escape sequences for writing directly to scrollback.
    pub fn as_scrollback_string(&self, width: u16) -> String {
        let height = Self::TOTAL_ROWS;
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        self.render(area, &mut buf);
        let bytes = crate::tui::ansi_serialize::buffer_rows_to_ansi(&buf, 0..height);
        String::from_utf8(bytes).unwrap_or_default()
    }
}

/// Center `s` in `width` columns, using char count for multibyte safety.
/// Odd padding: left gets the extra space (matches Claude Code reference layout).
fn center_pad(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        return s.to_string();
    }
    let pad = width - len;
    let right_pad = pad / 2;
    let left_pad = pad - right_pad;
    format!("{}{}{}", " ".repeat(left_pad), s, " ".repeat(right_pad))
}

impl<'a> Widget for WelcomeWidget<'a> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let info_alphas = [1.0f64; Self::INFO_ROWS];
        self.render_with_alpha(area, buf, &info_alphas);
    }
}
