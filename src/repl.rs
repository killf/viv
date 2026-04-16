use std::io::{stdin, stdout, Read, Write};

use crate::llm::{LlmClient, LlmConfig, Message, ModelTier};
use crate::terminal::input::{InputParser, KeyEvent};
use crate::terminal::raw_mode::RawMode;

/// Start the REPL: print banner, enter raw mode, and loop reading + processing input.
pub fn run() -> crate::Result<()> {
    let config = LlmConfig::from_env()?;
    let client = LlmClient::new(config);

    let mut out = stdout();

    // Banner
    write!(out, "viv \u{2014} AI coding agent (type /exit or Ctrl+D to quit)\r\n")?;
    out.flush()?;

    // Enter raw mode — RAII guard restores terminal on drop
    let _raw = RawMode::enable(0)?;

    let mut messages: Vec<Message> = Vec::new();
    let mut editor = LineEditor::new();
    let mut parser = InputParser::new();

    // Show initial prompt
    // Green bold: \x1b[1;32m  Reset: \x1b[0m
    write!(out, "\x1b[1;32m> \x1b[0m")?;
    out.flush()?;

    let mut stdin_buf = [0u8; 256];

    loop {
        // Blocking read of raw bytes from stdin
        let n = stdin().read(&mut stdin_buf)?;
        if n == 0 {
            // EOF
            write!(out, "\r\nBye!\r\n")?;
            out.flush()?;
            return Ok(());
        }

        parser.feed(&stdin_buf[..n]);

        // Process all available key events from the parser
        while let Some(key) = parser.next_event() {
            let action = editor.handle_key(key);

            match action {
                EditAction::Submit(line) => {
                    // Move to next line after user input
                    write!(out, "\r\n")?;
                    out.flush()?;

                    // Skip empty lines — just redraw prompt
                    if line.trim().is_empty() {
                        write!(out, "\x1b[1;32m> \x1b[0m")?;
                        out.flush()?;
                        continue;
                    }

                    // Handle /exit command
                    if line.trim() == "/exit" {
                        write!(out, "Bye!\r\n")?;
                        out.flush()?;
                        return Ok(());
                    }

                    // Add user message to conversation history
                    messages.push(Message {
                        role: "user".into(),
                        content: line,
                    });

                    // Print "claude: " prefix in magenta bold
                    write!(out, "\x1b[1;35mclaude: \x1b[0m")?;
                    out.flush()?;

                    // Stream response from LLM
                    let mut response = String::new();
                    let stream_result = client.stream(&messages, ModelTier::Medium, |text| {
                        // In raw mode we must use \r\n for newlines
                        let replaced = text.replace('\n', "\r\n");
                        let _ = out.write_all(replaced.as_bytes());
                        let _ = out.flush();
                        response.push_str(text);
                    });

                    match stream_result {
                        Ok(full) => {
                            // Use the accumulated text from the callback; full == response
                            let _ = full; // already accumulated in `response`
                            messages.push(Message {
                                role: "assistant".into(),
                                content: response,
                            });
                        }
                        Err(e) => {
                            write!(out, "\r\n\x1b[1;31merror: {}\x1b[0m\r\n", e)?;
                        }
                    }

                    // Trailing newlines then fresh prompt
                    write!(out, "\r\n\r\n\x1b[1;32m> \x1b[0m")?;
                    out.flush()?;
                }

                EditAction::Exit => {
                    write!(out, "\r\nBye!\r\n")?;
                    out.flush()?;
                    return Ok(());
                }

                EditAction::Interrupt => {
                    // Clear current line and show a fresh prompt
                    editor.buf.clear();
                    editor.cursor = 0;
                    write!(out, "\r\x1b[2K\x1b[1;32m> \x1b[0m")?;
                    out.flush()?;
                }

                EditAction::Continue => {
                    // Redraw the current line with the cursor at the correct position
                    let cursor_col = 2 + editor.buf[..editor.cursor].chars().count();
                    write!(out, "\r\x1b[2K\x1b[1;32m> \x1b[0m{}", &editor.buf)?;
                    // Move cursor to correct column (1-based)
                    write!(out, "\x1b[{}G", cursor_col + 1)?;
                    out.flush()?;
                }
            }
        }
    }
}

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
