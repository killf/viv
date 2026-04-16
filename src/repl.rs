use crate::terminal::input::KeyEvent;

#[derive(Debug, PartialEq)]
pub enum EditAction {
    Continue,
    Submit(String),
    Exit,
    Interrupt,
}

pub struct LineEditor {
    pub buf: String,
    pub cursor: usize,
}

impl LineEditor {
    pub fn new() -> Self {
        LineEditor {
            buf: String::new(),
            cursor: 0,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> EditAction {
        match key {
            KeyEvent::Char(ch) => {
                self.buf.insert(self.cursor, ch);
                self.cursor += ch.len_utf8();
                EditAction::Continue
            }
            KeyEvent::Enter => {
                let line = self.buf.clone();
                self.buf.clear();
                self.cursor = 0;
                EditAction::Submit(line)
            }
            KeyEvent::Backspace => {
                if self.cursor > 0 {
                    // Find the char boundary before cursor
                    let prev = self.prev_char_boundary();
                    self.buf.drain(prev..self.cursor);
                    self.cursor = prev;
                }
                EditAction::Continue
            }
            KeyEvent::Delete => {
                if self.cursor < self.buf.len() {
                    let next = self.next_char_boundary();
                    self.buf.drain(self.cursor..next);
                }
                EditAction::Continue
            }
            KeyEvent::Left => {
                if self.cursor > 0 {
                    self.cursor = self.prev_char_boundary();
                }
                EditAction::Continue
            }
            KeyEvent::Right => {
                if self.cursor < self.buf.len() {
                    self.cursor = self.next_char_boundary();
                }
                EditAction::Continue
            }
            KeyEvent::Home => {
                self.cursor = 0;
                EditAction::Continue
            }
            KeyEvent::End => {
                self.cursor = self.buf.len();
                EditAction::Continue
            }
            KeyEvent::CtrlC => EditAction::Interrupt,
            KeyEvent::CtrlD => {
                if self.buf.is_empty() {
                    EditAction::Exit
                } else {
                    EditAction::Continue
                }
            }
            _ => EditAction::Continue,
        }
    }

    fn prev_char_boundary(&self) -> usize {
        let mut pos = self.cursor.saturating_sub(1);
        while pos > 0 && !self.buf.is_char_boundary(pos) {
            pos -= 1;
        }
        pos
    }

    fn next_char_boundary(&self) -> usize {
        let mut pos = self.cursor + 1;
        while pos < self.buf.len() && !self.buf.is_char_boundary(pos) {
            pos += 1;
        }
        pos
    }
}

impl Default for LineEditor {
    fn default() -> Self {
        Self::new()
    }
}
