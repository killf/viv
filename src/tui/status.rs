use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::style::theme;
use crate::tui::widget::Widget;

// Anthropic claude-sonnet-4-6 pricing (USD per million tokens, as of 2026-04)
const INPUT_PRICE_PER_M: f64 = 3.0;
const OUTPUT_PRICE_PER_M: f64 = 15.0;

/// Inputs for constructing a [`StatusWidget`] — decouples the widget's shape
/// from callers that hold transient rendering state (spinner frames, etc.).
pub struct StatusContext {
    pub cwd: String,
    pub branch: Option<String>,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub spinner_frame: Option<char>,
    pub spinner_verb: String,
}

pub struct StatusWidget {
    pub cwd: String,
    pub branch: Option<String>,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub spinner_frame: Option<char>,
    pub spinner_verb: String,
}

impl StatusWidget {
    pub fn estimate_cost(&self) -> f64 {
        (self.input_tokens as f64 / 1_000_000.0) * INPUT_PRICE_PER_M
            + (self.output_tokens as f64 / 1_000_000.0) * OUTPUT_PRICE_PER_M
    }

    /// Build a [`StatusWidget`] from a [`StatusContext`] by cloning its fields.
    pub fn from_context(ctx: &StatusContext) -> Self {
        StatusWidget {
            cwd: ctx.cwd.clone(),
            branch: ctx.branch.clone(),
            model: ctx.model.clone(),
            input_tokens: ctx.input_tokens,
            output_tokens: ctx.output_tokens,
            spinner_frame: ctx.spinner_frame,
            spinner_verb: ctx.spinner_verb.clone(),
        }
    }
}

impl StatusWidget {
    fn right_text(&self) -> String {
        let cost = self.estimate_cost();
        format!(
            "  {}  ↑ {}  ↓ {}  ~${:.3}",
            self.model, self.input_tokens, self.output_tokens, cost
        )
    }

    fn left_text(&self) -> String {
        match &self.branch {
            Some(b) => format!("  {}  ⎇ {}", self.cwd, b),
            None => format!("  {}", self.cwd),
        }
    }
}

impl Widget for StatusWidget {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        // Optional spinner prefix in Claude orange: "<glyph> <verb>… · "
        let mut left_x = area.x;
        if let Some(glyph) = self.spinner_frame {
            let prefix = format!("{} {}\u{2026} ", glyph, self.spinner_verb);
            buf.set_str(left_x, area.y, &prefix, Some(theme::CLAUDE), false);
            let prefix_w = prefix.chars().count() as u16;
            left_x = left_x.saturating_add(prefix_w);
            // separator dot
            let sep = "\u{00b7} ";
            buf.set_str(left_x, area.y, sep, Some(theme::DIM), false);
            let sep_w = sep.chars().count() as u16;
            left_x = left_x.saturating_add(sep_w);
        }

        // Left: cwd + branch
        let left = self.left_text();
        buf.set_str(left_x, area.y, &left, Some(theme::DIM), false);

        // Right: model + tokens
        let right = self.right_text();
        let right_len = right.chars().count() as u16;
        let right_x = area.x.saturating_add(area.width).saturating_sub(right_len);
        buf.set_str(right_x, area.y, &right, Some(theme::DIM), false);
    }
}
