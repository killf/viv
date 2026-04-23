use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::style::theme;
use crate::tui::widget::Widget;

const LOGO: [&str; 5] = [
    "       _       ",
    "__   _(_)_   __",
    "\\ \\ / / \\ \\ / /",
    " \\ V /| |\\ V / ",
    "  \\_/ |_| \\_/  ",
];

const LOGO_WIDTH: u16 = 15;
const GAP: u16 = 4;

pub struct WelcomeWidget<'a> {
    model: Option<&'a str>,
    cwd: &'a str,
    branch: Option<&'a str>,
}

impl<'a> WelcomeWidget<'a> {
    pub const HEIGHT: u16 = 5;

    /// Row delay in milliseconds between each info row starting.
    pub const ROW_DELAY_MS: u64 = 80;
    /// Fade-in duration in milliseconds per row.
    pub const FADE_DURATION_MS: u64 = 200;
    /// Number of info rows (the logo is rows 0-4, info rows are 5-9).
    pub const INFO_ROWS: usize = 5;
    /// Total number of rows (logo + info).
    pub const TOTAL_ROWS: u16 = 10;

    pub fn new(model: Option<&'a str>, cwd: &'a str, branch: Option<&'a str>) -> Self {
        WelcomeWidget { model, cwd, branch }
    }

    /// Render the welcome widget with per-row alpha values for the 5 info rows.
    /// `info_alphas` must have exactly `INFO_ROWS` entries.
    /// Each entry is in [0.0, 1.0]: alpha < 0.5 → DIM, alpha >= 0.5 → TEXT.
    pub fn render_with_alpha(&self, area: Rect, buf: &mut Buffer, info_alphas: &[f64]) {
        if area.is_empty() || area.height < Self::HEIGHT {
            return;
        }

        // Render logo (rows 0-4): always CLAUDE
        for (row, line) in LOGO.iter().enumerate() {
            let y = area.y + row as u16;
            if y >= area.y + area.height {
                break;
            }
            buf.set_str(area.x, y, line, Some(theme::CLAUDE), false);
        }

        // Render info to the right of logo
        let info_x = area.x + LOGO_WIDTH + GAP;
        if info_x >= area.x + area.width {
            return;
        }

        let label_width: u16 = 10;
        let info_lines = self.info_lines();

        for (row, (label, value)) in info_lines.iter().enumerate() {
            let y = area.y + row as u16;
            if y >= area.y + area.height {
                break;
            }

            // Label in CLAUDE orange, bold
            buf.set_str(info_x, y, label, Some(theme::CLAUDE), true);

            // Value color based on alpha
            let alpha = info_alphas
                .get(row)
                .copied()
                .unwrap_or(1.0);
            let val_fg = if alpha < 0.5 { theme::DIM } else { theme::TEXT };

            // Value in white
            let val_x = info_x + label_width;
            if val_x < area.x + area.width {
                buf.set_str(val_x, y, value, Some(val_fg), false);
            }
        }
    }

    fn info_lines(&self) -> [(&str, String); 5] {
        let model_val = self.model.unwrap_or("...").to_string();
        let cwd_val = self.cwd.to_string();
        let branch_val = self.branch.unwrap_or("-").to_string();

        let platform = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH,);

        let shell = std::env::var("SHELL")
            .ok()
            .and_then(|s| s.rsplit('/').next().map(|n| n.to_string()))
            .unwrap_or_else(|| "-".to_string());

        [
            ("Model:", model_val),
            ("CWD:", cwd_val),
            ("Branch:", branch_val),
            ("Platform:", platform),
            ("Shell:", shell),
        ]
    }

    /// Render the welcome widget into an ANSI-encoded string suitable for
    /// writing directly to scrollback. All info rows are fully visible.
    pub fn as_scrollback_string(&self, width: u16) -> String {
        let height = Self::TOTAL_ROWS;
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        self.render(area, &mut buf);
        let bytes = crate::tui::ansi_serialize::buffer_rows_to_ansi(&buf, 0..height);
        String::from_utf8(bytes).unwrap_or_default()
    }
}

impl<'a> Widget for WelcomeWidget<'a> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        // Default: all info rows fully visible (alpha = 1.0)
        let info_alphas = [1.0; Self::INFO_ROWS];
        self.render_with_alpha(area, buf, &info_alphas);
    }
}
