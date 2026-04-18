use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::style::theme;
use crate::tui::widget::Widget;

pub struct HeaderWidget {
    pub cwd: String,
    pub branch: Option<String>,
}

impl HeaderWidget {
    pub fn from_env() -> Self {
        let raw_cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "?".to_string());
        let home = std::env::var("HOME").unwrap_or_default();
        let cwd = if !home.is_empty() && raw_cwd.starts_with(&home) {
            format!("~{}", &raw_cwd[home.len()..])
        } else {
            raw_cwd
        };
        let branch = std::fs::read_to_string(".git/HEAD")
            .ok()
            .and_then(|s| parse_branch(&s));
        Self::from_path(&cwd, branch)
    }

    pub fn from_path(cwd: &str, branch: Option<String>) -> Self {
        let cwd = if cwd.chars().count() > 30 {
            let tail: String = cwd
                .chars()
                .rev()
                .take(29)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            format!("…{}", tail)
        } else {
            cwd.to_string()
        };
        HeaderWidget { cwd, branch }
    }
}

pub fn parse_branch(head_content: &str) -> Option<String> {
    head_content
        .trim()
        .strip_prefix("ref: refs/heads/")
        .map(|b| b.to_string())
}

impl Widget for HeaderWidget {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let text = match &self.branch {
            Some(b) => format!("  {}  ⎇ {}", self.cwd, b),
            None => format!("  {}", self.cwd),
        };
        buf.set_str(area.x, area.y, &text, Some(theme::DIM), false);
    }
}
