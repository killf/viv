use crate::terminal::buffer::{char_width, Buffer, Rect};
use crate::terminal::style::Color;
use crate::tui::widget::Widget;

/// A single-line text input widget with a prompt, cursor, and horizontal scrolling.
pub struct InputWidget<'a> {
    pub content: &'a str,
    pub cursor: usize,
    pub prompt: &'a str,
    pub prompt_fg: Option<Color>,
}

impl<'a> InputWidget<'a> {
    pub fn new(content: &'a str, cursor: usize, prompt: &'a str) -> Self {
        InputWidget { content, cursor, prompt, prompt_fg: None }
    }

    /// Builder: set prompt foreground color.
    pub fn prompt_fg(mut self, fg: Color) -> Self {
        self.prompt_fg = Some(fg);
        self
    }

    /// Returns the absolute (col, row) position where the cursor should be placed.
    pub fn cursor_position(&self, area: Rect) -> (u16, u16) {
        let prompt_width: u16 = self.prompt.chars().map(char_width).sum();
        let cursor_width: u16 = self.content[..self.cursor.min(self.content.len())]
            .chars().map(char_width).sum();
        let available = area.width as usize;
        let total = prompt_width + cursor_width;
        let scroll = self.scroll_offset(area);
        let col = area.x + (total as usize).saturating_sub(scroll).min(available) as u16;
        (col, area.y)
    }

    fn scroll_offset(&self, area: Rect) -> usize {
        let prompt_width: usize = self.prompt.chars().map(|c| char_width(c) as usize).sum();
        let cursor_width: usize = self.content[..self.cursor.min(self.content.len())]
            .chars().map(|c| char_width(c) as usize).sum();
        let available = area.width as usize;
        let total = prompt_width + cursor_width;
        if total < available { 0 } else { total + 1 - available }
    }
}

impl Widget for InputWidget<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let scroll = self.scroll_offset(area);
        let available = area.width;

        // Build unified stream: (char, is_prompt)
        let prompt_chars: Vec<char> = self.prompt.chars().collect();
        let content_chars: Vec<char> = self.content.chars().collect();

        let mut col = area.x;
        let mut logical_col: usize = 0; // display column counter

        for (ch, is_prompt) in prompt_chars.iter().map(|&c| (c, true))
            .chain(content_chars.iter().map(|&c| (c, false)))
        {
            let w = char_width(ch) as usize;
            if w == 0 { continue; }

            if logical_col + w <= scroll {
                logical_col += w;
                continue;
            }
            if col >= area.x + available {
                break;
            }

            let fg = if is_prompt { self.prompt_fg } else { None };
            let cell = buf.get_mut(col, area.y);
            cell.ch = ch;
            cell.fg = fg;
            cell.bold = false;

            if w == 2 && col + 1 < area.x + available {
                let cell2 = buf.get_mut(col + 1, area.y);
                cell2.ch = '\0';
                cell2.fg = fg;
                cell2.bold = false;
            }

            col += w as u16;
            logical_col += w;
        }
    }
}
