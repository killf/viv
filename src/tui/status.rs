use crate::terminal::buffer::{Buffer, Rect};
use crate::terminal::style::theme;
use crate::tui::widget::Widget;

// Anthropic claude-sonnet-4-6 pricing (USD per million tokens, as of 2026-04)
const INPUT_PRICE_PER_M: f64 = 3.0;
const OUTPUT_PRICE_PER_M: f64 = 15.0;

pub struct StatusWidget {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl StatusWidget {
    pub fn estimate_cost(&self) -> f64 {
        (self.input_tokens as f64 / 1_000_000.0) * INPUT_PRICE_PER_M
            + (self.output_tokens as f64 / 1_000_000.0) * OUTPUT_PRICE_PER_M
    }
}

impl Widget for StatusWidget {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() { return; }
        let cost = self.estimate_cost();
        let text = format!(
            "  {}  ↑ {}  ↓ {}  ~${:.3}",
            self.model, self.input_tokens, self.output_tokens, cost
        );
        buf.set_str(area.x, area.y, &text, Some(theme::DIM), false);
    }
}
