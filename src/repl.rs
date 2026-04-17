use std::sync::Arc;
use crate::agent::context::AgentContext;
use crate::agent::evolution::evolve_from_session;
use crate::agent::run::run_agent;
use crate::llm::{LLMClient, LLMConfig};
use crate::terminal::backend::{Backend, LinuxBackend};
use crate::terminal::buffer::char_width;
use crate::terminal::events::{Event, EventLoop};
use crate::terminal::input::KeyEvent;
use crate::terminal::style::theme;
use crate::tui::block::{Block, BorderSides, BorderStyle};
use crate::tui::input::InputWidget;
use crate::tui::layout::{Constraint, Direction, Layout};
use crate::tui::message_style::{
    format_assistant_message, format_error_message, format_user_message, format_welcome,
};
use crate::tui::paragraph::{Line, Paragraph, Span};
use crate::tui::renderer::Renderer;
use crate::tui::spinner::{random_verb, Spinner};
use crate::tui::widget::Widget;

/// Start the REPL: initialize TUI, enter raw mode, and run the event loop.
pub fn run() -> crate::Result<()> {
    let config = LLMConfig::from_env()?;
    let client = LLMClient::new(config);
    let viv_dir = std::path::PathBuf::from(".viv/memory");
    let mut agent_ctx = AgentContext::new(Arc::new(client), viv_dir)?;

    let mut backend = LinuxBackend::new();
    backend.enter_alt_screen()?;
    backend.enable_raw_mode()?;
    backend.flush()?;

    let size = backend.size()?;
    let mut renderer = Renderer::new(size);
    let mut event_loop = EventLoop::new()?;
    let mut editor = LineEditor::new();

    let mut history_lines: Vec<Line> = Vec::new();
    let mut scroll: u16 = 0;
    let mut dirty: bool = true;
    let mut last_cursor: (u16, u16) = (0, 0);

    // Minimal welcome (Claude Code style: subtle, single line)
    history_lines.push(format_welcome());
    history_lines.push(Line::raw(""));

    loop {
        if dirty {
            // Render frame
            render_frame(
                &mut renderer,
                &history_lines,
                &editor,
                scroll,
            );
            renderer.flush(&mut backend)?;

            // Position cursor at the input widget location
            let area = renderer.area();
            let chunks = main_layout().split(area);
            let input_block = Block::new()
                .border(BorderStyle::Rounded)
                .borders(BorderSides::HORIZONTAL)
                .border_fg(theme::DIM);
            let input_inner = input_block.inner(chunks[1]);
            let editor_content = editor.content();
            let input_widget = InputWidget::new(&editor_content, editor.cursor_offset(), "\u{276F} ")
                .prompt_fg(theme::CLAUDE);
            let (cx, cy) = input_widget.cursor_position(input_inner);
            if (cx, cy) != last_cursor {
                backend.move_cursor(cy, cx)?;
                last_cursor = (cx, cy);
            }
            backend.show_cursor()?;
            backend.flush()?;

            dirty = false;
        }

        // Poll events (~60fps)
        let events = event_loop.poll(16)?;
        for event in events {
            match event {
                Event::Key(key) => {
                    dirty = true;
                    let action = editor.handle_key(key);
                    match action {
                        EditAction::Submit(line) => {
                            // Skip empty lines
                            if line.trim().is_empty() {
                                continue;
                            }

                            // Handle /exit command
                            if line.trim() == "/exit" {
                                let mut idx = agent_ctx.index.lock().unwrap();
                                let _ = evolve_from_session(
                                    &agent_ctx.messages, &agent_ctx.store, &mut idx, &agent_ctx.llm,
                                );
                                drop(idx);
                                backend.disable_raw_mode()?;
                                backend.leave_alt_screen()?;
                                backend.write(b"Bye!\n")?;
                                backend.flush()?;
                                return Ok(());
                            }

                            // User message
                            history_lines.push(format_user_message(&line));

                            // Auto-scroll to bottom before streaming
                            scroll = compute_max_scroll(&history_lines, &renderer);

                            // Render before streaming starts
                            render_frame(&mut renderer, &history_lines, &editor, scroll);
                            renderer.flush(&mut backend)?;
                            backend.flush()?;

                            // Stream LLM response
                            let mut response = String::new();

                            // Spinner placeholder shown until first chunk arrives
                            let spinner = Spinner::new();
                            let verb = random_verb(
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_millis() as u64)
                                    .unwrap_or(0),
                            );
                            let stream_start = std::time::Instant::now();

                            history_lines.push(Line::from_spans(vec![
                                Span::styled(
                                    format!("{} ", spinner.frame_at(0)),
                                    theme::CLAUDE,
                                    false,
                                ),
                                Span::styled(format!("{}\u{2026}", verb), theme::DIM, false),
                            ]));
                            let response_line_idx = history_lines.len() - 1;

                            scroll = compute_max_scroll(&history_lines, &renderer);
                            render_frame(&mut renderer, &history_lines, &editor, scroll);
                            renderer.flush(&mut backend)?;
                            backend.flush()?;

                            let mut ask_fn = |tool_name: &str, tool_input: &crate::json::JsonValue| -> bool {
                                use std::io::{Read, Write};
                                let summary = format_tool_summary(tool_input);
                                let prompt = format!(
                                    "\r\n\x1b[33m Allow {}({})? [y/n] \x1b[0m",
                                    tool_name, summary
                                );
                                let _ = std::io::stdout().write_all(prompt.as_bytes());
                                let _ = std::io::stdout().flush();
                                let mut buf = [0u8; 1];
                                loop {
                                    match std::io::stdin().lock().read(&mut buf) {
                                        Ok(1) => match buf[0] {
                                            b'y' | b'Y' => {
                                                let _ = std::io::stdout().write_all(b"y\r\n");
                                                let _ = std::io::stdout().flush();
                                                return true;
                                            }
                                            _ => {
                                                let _ = std::io::stdout().write_all(b"n\r\n");
                                                let _ = std::io::stdout().flush();
                                                return false;
                                            }
                                        },
                                        _ => return false,
                                    }
                                }
                            };
                            let agent_result =
                                run_agent(line, &mut agent_ctx, &mut ask_fn, |text| {
                                    response.push_str(text);

                                    if response.trim().is_empty() {
                                        // Still waiting — animate the spinner
                                        let elapsed =
                                            stream_start.elapsed().as_millis() as u64;
                                        history_lines.truncate(response_line_idx);
                                        history_lines.push(Line::from_spans(vec![
                                            Span::styled(
                                                format!("{} ", spinner.frame_at(elapsed)),
                                                theme::CLAUDE,
                                                false,
                                            ),
                                            Span::styled(
                                                format!("{}\u{2026}", verb),
                                                theme::DIM,
                                                false,
                                            ),
                                        ]));
                                    } else {
                                        // First real text: replace spinner with the message
                                        history_lines.truncate(response_line_idx);
                                        history_lines.extend(format_assistant_message(&response));
                                    }

                                    scroll = compute_max_scroll(&history_lines, &renderer);
                                    render_frame(
                                        &mut renderer,
                                        &history_lines,
                                        &editor,
                                        scroll,
                                    );
                                    let _ = renderer.flush(&mut backend);
                                    let _ = backend.flush();
                                });

                            match agent_result {
                                Ok(_output) => {
                                    history_lines.truncate(response_line_idx);
                                    history_lines.extend(format_assistant_message(&response));
                                }
                                Err(e) => {
                                    history_lines.truncate(response_line_idx);
                                    history_lines
                                        .extend(format_error_message(&format!("error: {}", e)));
                                }
                            }

                            history_lines.push(Line::raw(""));

                            scroll = compute_max_scroll(&history_lines, &renderer);
                        }

                        EditAction::Exit => {
                            let mut idx = agent_ctx.index.lock().unwrap();
                            let _ = evolve_from_session(
                                &agent_ctx.messages, &agent_ctx.store, &mut idx, &agent_ctx.llm,
                            );
                            drop(idx);
                            backend.disable_raw_mode()?;
                            backend.leave_alt_screen()?;
                            backend.write(b"Bye!\n")?;
                            backend.flush()?;
                            return Ok(());
                        }

                        EditAction::Interrupt => {
                            editor.lines = vec![String::new()];
                            editor.row = 0;
                            editor.col = 0;
                        }

                        EditAction::Continue => {
                            // Just re-render on next iteration
                        }
                    }
                }
                Event::Resize(new_size) => {
                    renderer.resize(new_size);
                    scroll = compute_max_scroll(&history_lines, &renderer);
                    dirty = true;
                }
                Event::Tick => {}
            }
        }
    }
}

fn format_tool_summary(input: &crate::json::JsonValue) -> String {
    match input {
        crate::json::JsonValue::Object(pairs) => pairs
            .iter()
            .take(2)
            .map(|(k, v)| {
                let val = match v {
                    crate::json::JsonValue::Str(s) => {
                        format!("\"{}\"", s.chars().take(40).collect::<String>())
                    }
                    other => format!("{}", other).chars().take(40).collect::<String>(),
                };
                format!("{}={}", k, val)
            })
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}

/// Build the main vertical layout: conversation (Fill) + input (Fixed 3) + footer (Fixed 1).
///
/// The input box uses only top + bottom borders (Claude Code style), so it
/// reserves 3 rows: top line, input content, bottom line.
fn main_layout() -> Layout {
    Layout::new(Direction::Vertical).constraints(vec![
        Constraint::Fill,
        Constraint::Fixed(3),
        Constraint::Fixed(1),
    ])
}

/// Render a full frame.
fn render_frame(
    renderer: &mut Renderer,
    history_lines: &[Line],
    editor: &LineEditor,
    scroll: u16,
) {
    let area = renderer.area();
    let chunks = main_layout().split(area);

    let buf = renderer.buffer_mut();

    // Conversation history
    let paragraph = Paragraph::new(history_lines.to_vec()).scroll(scroll);
    paragraph.render(chunks[0], buf);

    // Input box: top + bottom rounded borders only, dim gray
    let input_block = Block::new()
        .border(BorderStyle::Rounded)
        .borders(BorderSides::HORIZONTAL)
        .border_fg(theme::DIM);
    let input_inner = input_block.inner(chunks[1]);
    input_block.render(chunks[1], buf);

    // Input widget with ❯ prompt (Claude orange)
    let editor_content = editor.content();
    let input_widget =
        InputWidget::new(&editor_content, editor.cursor_offset(), "\u{276F} ").prompt_fg(theme::CLAUDE);
    input_widget.render(input_inner, buf);

    // Footer line (dim): keybind hints, Claude Code style
    let footer = Line::from_spans(vec![
        Span::styled("  ? for shortcuts", theme::DIM, false),
    ]);
    let footer_para = Paragraph::new(vec![footer]);
    footer_para.render(chunks[2], buf);
}

/// Compute the maximum scroll offset so the last line of history is visible
/// at the bottom of the conversation area.
fn compute_max_scroll(history_lines: &[Line], renderer: &Renderer) -> u16 {
    let area = renderer.area();
    let chunks = main_layout().split(area);
    let conv_height = chunks[0].height as usize;
    let conv_width = chunks[0].width as usize;

    if conv_width == 0 || conv_height == 0 {
        return 0;
    }

    // Count total physical (wrapped) lines
    let mut total_rows: usize = 0;
    for line in history_lines {
        let wrapped = count_wrapped_rows(line, conv_width);
        total_rows += wrapped;
    }

    if total_rows > conv_height {
        (total_rows - conv_height) as u16
    } else {
        0
    }
}

/// Count how many physical rows a Line will occupy after word-wrapping to `width`.
fn count_wrapped_rows(line: &Line, width: usize) -> usize {
    if width == 0 {
        return 0;
    }

    let total_width: usize = line.spans.iter()
        .flat_map(|s| s.text.chars())
        .map(|c| char_width(c) as usize)
        .sum();

    if total_width == 0 {
        return 1; // empty line still takes one row
    }

    total_width.div_ceil(width)
}

#[derive(Debug, PartialEq)]
pub enum EditAction {
    Continue,
    Submit(String),
    Exit,
    Interrupt,
}

pub struct LineEditor {
    pub lines: Vec<String>,
    pub row: usize,
    pub col: usize,
}

impl LineEditor {
    pub fn new() -> Self {
        LineEditor { lines: vec![String::new()], row: 0, col: 0 }
    }

    pub fn content(&self) -> String {
        self.lines.join("\n")
    }

    pub fn cursor_offset(&self) -> usize {
        let prefix: usize = self.lines[..self.row].iter().map(|l| l.len() + 1).sum();
        prefix + self.col
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> EditAction {
        match key {
            KeyEvent::Char(ch) => {
                self.lines[self.row].insert(self.col, ch);
                self.col += ch.len_utf8();
                EditAction::Continue
            }
            KeyEvent::ShiftEnter => {
                let rest = self.lines[self.row].split_off(self.col);
                self.lines.insert(self.row + 1, rest);
                self.row += 1;
                self.col = 0;
                EditAction::Continue
            }
            KeyEvent::Enter => {
                let content = self.content();
                self.lines = vec![String::new()];
                self.row = 0;
                self.col = 0;
                EditAction::Submit(content)
            }
            KeyEvent::Backspace => {
                if self.col > 0 {
                    let prev = self.prev_char_boundary();
                    self.lines[self.row].drain(prev..self.col);
                    self.col = prev;
                } else if self.row > 0 {
                    let current = self.lines.remove(self.row);
                    self.row -= 1;
                    self.col = self.lines[self.row].len();
                    self.lines[self.row].push_str(&current);
                }
                EditAction::Continue
            }
            KeyEvent::Delete => {
                if self.col < self.lines[self.row].len() {
                    let next = self.next_char_boundary();
                    self.lines[self.row].drain(self.col..next);
                } else if self.row + 1 < self.lines.len() {
                    let next_line = self.lines.remove(self.row + 1);
                    self.lines[self.row].push_str(&next_line);
                }
                EditAction::Continue
            }
            KeyEvent::Left => {
                if self.col > 0 {
                    self.col = self.prev_char_boundary();
                } else if self.row > 0 {
                    self.row -= 1;
                    self.col = self.lines[self.row].len();
                }
                EditAction::Continue
            }
            KeyEvent::Right => {
                if self.col < self.lines[self.row].len() {
                    self.col = self.next_char_boundary();
                } else if self.row + 1 < self.lines.len() {
                    self.row += 1;
                    self.col = 0;
                }
                EditAction::Continue
            }
            KeyEvent::Up => {
                if self.row > 0 {
                    self.row -= 1;
                    self.col = self.col.min(self.lines[self.row].len());
                    while self.col > 0 && !self.lines[self.row].is_char_boundary(self.col) {
                        self.col -= 1;
                    }
                }
                EditAction::Continue
            }
            KeyEvent::Down => {
                if self.row + 1 < self.lines.len() {
                    self.row += 1;
                    self.col = self.col.min(self.lines[self.row].len());
                    while self.col > 0 && !self.lines[self.row].is_char_boundary(self.col) {
                        self.col -= 1;
                    }
                }
                EditAction::Continue
            }
            KeyEvent::Home => { self.col = 0; EditAction::Continue }
            KeyEvent::End => {
                self.col = self.lines[self.row].len();
                EditAction::Continue
            }
            KeyEvent::CtrlC => EditAction::Interrupt,
            KeyEvent::CtrlD => {
                if self.is_empty() { EditAction::Exit } else { EditAction::Continue }
            }
            _ => EditAction::Continue,
        }
    }

    fn prev_char_boundary(&self) -> usize {
        let mut pos = self.col.saturating_sub(1);
        while pos > 0 && !self.lines[self.row].is_char_boundary(pos) { pos -= 1; }
        pos
    }

    fn next_char_boundary(&self) -> usize {
        let line = &self.lines[self.row];
        let mut pos = self.col + 1;
        while pos < line.len() && !line.is_char_boundary(pos) { pos += 1; }
        pos
    }
}

impl Default for LineEditor {
    fn default() -> Self { Self::new() }
}
