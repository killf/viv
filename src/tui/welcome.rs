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

    pub fn new(model: Option<&'a str>, cwd: &'a str, branch: Option<&'a str>) -> Self {
        WelcomeWidget { model, cwd, branch }
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
}

impl<'a> Widget for WelcomeWidget<'a> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() || area.height < Self::HEIGHT {
            return;
        }

        // Render logo
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

            // Value in white
            let val_x = info_x + label_width;
            if val_x < area.x + area.width {
                buf.set_str(val_x, y, value, Some(theme::TEXT), false);
            }
        }
    }
}
