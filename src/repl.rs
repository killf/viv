use std::sync::Arc;
use crate::agent::context::AgentContext;
use crate::agent::evolution::evolve_from_session;
use crate::agent::run::run_agent;
use crate::llm::{LLMClient, LLMConfig};
use crate::core::terminal::backend::{Backend, LinuxBackend};
use crate::core::terminal::buffer::char_width;
use crate::core::terminal::events::{Event, EventLoop};
use crate::core::terminal::input::KeyEvent;
use crate::core::terminal::style::theme;
use crate::tui::block::{Block, BorderSides, BorderStyle};
use crate::tui::header::HeaderWidget;
use crate::tui::input::InputWidget;
use crate::tui::layout::{Constraint, Direction, Layout};
use crate::tui::message_style::{
    format_assistant_message, format_error_message, format_user_message, format_welcome,
};
use crate::tui::permission::{render_permission_pending, render_permission_result};
use crate::tui::paragraph::{Line, Paragraph, Span};
use crate::tui::renderer::Renderer;
use crate::tui::spinner::{random_verb, Spinner};
use crate::tui::status::StatusWidget;
use crate::tui::widget::Widget;

/// Start the REPL: initialize TUI, enter raw mode, and run the event loop.
pub fn run() -> crate::Result<()> {
    let config = LLMConfig::from_env()?;
    let model_tier = crate::agent::context::AgentConfig::default().model_tier;
    let model_name = config.model(model_tier.clone()).to_string();
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
    let header = HeaderWidget::from_env();
    history_lines.push(format_welcome(&header.cwd, header.branch.as_deref()));
    history_lines.push(Line::raw(""));

    loop {
        if dirty {
            // Render frame
            render_frame(
                &mut renderer,
                &history_lines,
                &editor,
                scroll,
                &model_name,
                agent_ctx.input_tokens,
                agent_ctx.output_tokens,
                &header,
            );
            renderer.flush(&mut backend)?;

            // Position cursor at the input widget location
            let area = renderer.area();
            let input_height = (editor.line_count() as u16 + 2).min(8).max(3);
            let chunks = main_layout(input_height).split(area);
            let input_block = Block::new()
                .border(BorderStyle::Rounded)
                .borders(BorderSides::HORIZONTAL)
                .border_fg(theme::DIM);
            let input_inner = input_block.inner(chunks[2]);
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
                            scroll = compute_max_scroll(&history_lines, &renderer, &editor);

                            // Render before streaming starts
                            render_frame(&mut renderer, &history_lines, &editor, scroll, &model_name, agent_ctx.input_tokens, agent_ctx.output_tokens, &header);
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

                            scroll = compute_max_scroll(&history_lines, &renderer, &editor);
                            render_frame(&mut renderer, &history_lines, &editor, scroll, &model_name, agent_ctx.input_tokens, agent_ctx.output_tokens, &header);
                            renderer.flush(&mut backend)?;
                            backend.flush()?;

                            // SAFETY: ask_fn and on_text are never called concurrently;
                            // both run in the same thread during run_agent, and ask_fn
                            // is only called between tool calls (not during SSE streaming).
                            // We hoist raw pointers before constructing either closure so
                            // that the borrow checker sees them as independent borrows.
                            let hl_ptr = &mut history_lines as *mut Vec<Line>;
                            let rend_ptr = &mut renderer as *mut Renderer;
                            let back_ptr = &mut backend as *mut LinuxBackend;
                            let scroll_ptr = &mut scroll as *mut u16;
                            let ctx_ptr = &mut agent_ctx as *mut AgentContext;

                            let model_name_clone = model_name.clone();
                            let header_clone = &header;
                            let _ask_fn = |tool_name: &str, tool_input: &crate::core::json::JsonValue| -> bool {
                                let summary = format_tool_summary(tool_input);

                                // SAFETY: same invariants as ask_fn above — single-threaded, not called concurrently.
                                let hl = unsafe { &mut *hl_ptr };
                                let rend = unsafe { &mut *rend_ptr };
                                let back = unsafe { &mut *back_ptr };
                                let scr = unsafe { &mut *scroll_ptr };
                                let ctx = unsafe { &mut *ctx_ptr };

                                hl.push(render_permission_pending(tool_name, &summary));
                                let perm_line_idx = hl.len() - 1;

                                *scr = compute_max_scroll(hl, rend, &editor);
                                render_frame(rend, hl, &editor, *scr, &model_name_clone, ctx.input_tokens, ctx.output_tokens, header_clone);
                                let _ = rend.flush(back);
                                let _ = back.flush();

                                // Block-read a single byte from stdin.
                                // The EventLoop set stdin to O_NONBLOCK; we temporarily
                                // switch back to blocking mode for this read.
                                let allowed = blocking_read_yn();

                                // Replace pending line with a result line
                                hl[perm_line_idx] = render_permission_result(tool_name, &summary, allowed);

                                *scr = compute_max_scroll(hl, rend, &editor);
                                render_frame(rend, hl, &editor, *scr, &model_name_clone, ctx.input_tokens, ctx.output_tokens, header_clone);
                                let _ = rend.flush(back);
                                let _ = back.flush();

                                allowed
                            };
                            let agent_result =
                                run_agent(line, &mut agent_ctx, |text| {
                                    response.push_str(text);

                                    let hl = unsafe { &mut *hl_ptr };
                                    let rend = unsafe { &mut *rend_ptr };
                                    let back = unsafe { &mut *back_ptr };
                                    let scr = unsafe { &mut *scroll_ptr };
                                    let ctx = unsafe { &mut *ctx_ptr };
                                    let header_ref = &header;

                                    if response.trim().is_empty() {
                                        // Still waiting — animate the spinner
                                        let elapsed =
                                            stream_start.elapsed().as_millis() as u64;
                                        hl.truncate(response_line_idx);
                                        hl.push(Line::from_spans(vec![
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
                                        hl.truncate(response_line_idx);
                                        hl.extend(format_assistant_message(&response));
                                    }

                                    *scr = compute_max_scroll(hl, rend, &editor);
                                    render_frame(rend, hl, &editor, *scr, &model_name_clone, ctx.input_tokens, ctx.output_tokens, header_ref);
                                    let _ = rend.flush(back);
                                    let _ = back.flush();
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

                            scroll = compute_max_scroll(&history_lines, &renderer, &editor);
                            render_frame(
                                &mut renderer,
                                &history_lines,
                                &editor,
                                scroll,
                                &model_name,
                                agent_ctx.input_tokens,
                                agent_ctx.output_tokens,
                                &header,
                            );
                            renderer.flush(&mut backend)?;

                            history_lines.push(Line::raw(""));

                            scroll = compute_max_scroll(&history_lines, &renderer, &editor);
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
                    scroll = compute_max_scroll(&history_lines, &renderer, &editor);
                    dirty = true;
                }
                Event::Tick => {}
            }
        }
    }
}

/// Read a single keypress from stdin in blocking mode, returning true for y/Y.
///
/// The EventLoop sets stdin to O_NONBLOCK. We temporarily restore blocking mode,
/// read one byte, then set non-blocking again. This is safe because ask_fn is
/// called between tool calls (not during SSE streaming) on the same thread.
fn blocking_read_yn() -> bool {
    // FFI: toggle O_NONBLOCK on stdin (fd 0)
    const F_GETFL: i32 = 3;
    const F_SETFL: i32 = 4;
    const O_NONBLOCK: i32 = 0o4000;

    unsafe extern "C" {
        fn fcntl(fd: i32, cmd: i32, ...) -> i32;
        fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    }

    // Save current flags and clear O_NONBLOCK
    let flags = unsafe { fcntl(0, F_GETFL) };
    if flags >= 0 {
        unsafe { fcntl(0, F_SETFL, flags & !O_NONBLOCK) };
    }

    let mut buf = [0u8; 1];
    let n = unsafe { read(0, buf.as_mut_ptr(), 1) };

    // Restore non-blocking mode
    if flags >= 0 {
        unsafe { fcntl(0, F_SETFL, flags) };
    }

    if n == 1 {
        matches!(buf[0], b'y' | b'Y')
    } else {
        false
    }
}

fn format_tool_summary(input: &crate::core::json::JsonValue) -> String {
    match input {
        crate::core::json::JsonValue::Object(pairs) => pairs
            .iter()
            .take(2)
            .map(|(k, v)| {
                let val = match v {
                    crate::core::json::JsonValue::Str(s) => {
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

/// Build the main vertical layout: header (1) + conversation (Fill) + input (dynamic) + status (1).
///
/// `input_height` = min(editor.line_count() + 2, 8), accounting for top+bottom borders.
fn main_layout(input_height: u16) -> Layout {
    Layout::new(Direction::Vertical).constraints(vec![
        Constraint::Fixed(1),
        Constraint::Fill,
        Constraint::Fixed(input_height),
        Constraint::Fixed(1),
    ])
}

/// Render a full frame.
fn render_frame(
    renderer: &mut Renderer,
    history_lines: &[Line],
    editor: &LineEditor,
    scroll: u16,
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
    header: &HeaderWidget,
) {
    let area = renderer.area();
    let input_height = (editor.line_count() as u16 + 2).min(8).max(3);
    let chunks = main_layout(input_height).split(area);
    // chunks: [0]=header, [1]=conversation, [2]=input, [3]=status

    let buf = renderer.buffer_mut();

    // Header bar
    header.render(chunks[0], buf);

    // Conversation history
    let paragraph = Paragraph::new(history_lines.to_vec()).scroll(scroll);
    paragraph.render(chunks[1], buf);

    // Input box: top + bottom rounded borders only, dim gray
    let input_block = Block::new()
        .border(BorderStyle::Rounded)
        .borders(BorderSides::HORIZONTAL)
        .border_fg(theme::DIM);
    let input_inner = input_block.inner(chunks[2]);
    input_block.render(chunks[2], buf);

    // Input widget with ❯ prompt (Claude orange)
    let editor_content = editor.content();
    let input_widget =
        InputWidget::new(&editor_content, editor.cursor_offset(), "\u{276F} ").prompt_fg(theme::CLAUDE);
    input_widget.render(input_inner, buf);

    // Status bar
    let status = StatusWidget {
        model: model.to_string(),
        input_tokens,
        output_tokens,
    };
    status.render(chunks[3], buf);
}

/// Compute the maximum scroll offset so the last line of history is visible
/// at the bottom of the conversation area.
fn compute_max_scroll(history_lines: &[Line], renderer: &Renderer, editor: &LineEditor) -> u16 {
    let area = renderer.area();
    let input_height = (editor.line_count() as u16 + 2).min(8).max(3);
    let chunks = main_layout(input_height).split(area);
    let conv_height = chunks[1].height as usize;
    let conv_width = chunks[1].width as usize;

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
