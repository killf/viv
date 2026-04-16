use crate::terminal::buffer::{Buffer, Rect};
use crate::tui::widget::Widget;

/// A single-line text input widget with a prompt, cursor, and horizontal scrolling.
pub struct InputWidget<'a> {
    pub content: &'a str,
    pub cursor: usize,
    pub prompt: &'a str,
    pub prompt_fg: Option<u8>,
}

impl<'a> InputWidget<'a> {
    pub fn new(content: &'a str, cursor: usize, prompt: &'a str) -> Self {
        InputWidget { content, cursor, prompt, prompt_fg: None }
    }

    /// Builder: set prompt foreground color.
    pub fn prompt_fg(mut self, fg: u8) -> Self {
        self.prompt_fg = Some(fg);
        self
    }

    /// Returns the absolute (col, row) position where the cursor should be placed.
    ///
    /// Accounts for prompt length and horizontal scrolling.
    pub fn cursor_position(&self, area: Rect) -> (u16, u16) {
        let prompt_len = self.prompt.chars().count();
        let available = area.width as usize;

        // cursor byte offset → char index
        let cursor_char = self.content[..self.cursor.min(self.content.len())].chars().count();

        let total_chars = prompt_len + cursor_char;

        let scroll = self.scroll_offset(area);

        // Absolute column = area.x + (total_chars - scroll), clamped to area
        let col = area.x + (total_chars.saturating_sub(scroll)).min(available) as u16;
        let row = area.y;
        (col, row)
    }

    /// Compute how many chars to skip from the left so the cursor stays visible.
    fn scroll_offset(&self, area: Rect) -> usize {
        let prompt_len = self.prompt.chars().count();
        let available = area.width as usize;
        let cursor_char = self.content[..self.cursor.min(self.content.len())].chars().count();
        let total_pos = prompt_len + cursor_char; // position from left edge (0-based)

        if total_pos < available {
            // Cursor fits without scrolling
            0
        } else {
            // Scroll so cursor is at the last column of the visible area
            total_pos + 1 - available
        }
    }
}

impl Widget for InputWidget<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let scroll = self.scroll_offset(area);
        let available = area.width as usize;

        // Build the full line: prompt chars + content chars
        let prompt_chars: Vec<char> = self.prompt.chars().collect();
        let content_chars: Vec<char> = self.content.chars().collect();

        let mut col = area.x;
        let mut rendered = 0usize;

        // Walk through prompt + content as a unified char stream, applying scroll
        let total_source: Vec<(char, bool)> = prompt_chars
            .iter()
            .map(|&c| (c, true))
            .chain(content_chars.iter().map(|&c| (c, false)))
            .collect();

        for (logical_idx, (ch, is_prompt)) in total_source.iter().enumerate() {
            if logical_idx < scroll {
                continue;
            }
            if rendered >= available {
                break;
            }
            let fg = if *is_prompt { self.prompt_fg } else { None };
            let cell = buf.get_mut(col, area.y);
            cell.ch = *ch;
            cell.fg = fg;
            cell.bold = false;
            col += 1;
            rendered += 1;
        }
    }
}
