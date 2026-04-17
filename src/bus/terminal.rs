use std::sync::mpsc::{Receiver, Sender};

use crate::bus::{AgentEvent, AgentMessage};
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
use crate::tui::paragraph::{Line, Paragraph, Span};
use crate::tui::permission::{render_permission_pending, render_permission_result};
use crate::tui::renderer::Renderer;
use crate::tui::spinner::{random_verb, Spinner};
use crate::tui::status::StatusWidget;
use crate::tui::widget::Widget;

// ─────────────────────────────────────────────────────────────────────────────
// UiAction
// ─────────────────────────────────────────────────────────────────────────────

enum UiAction {
    Quit,
}

// ─────────────────────────────────────────────────────────────────────────────
// TerminalUI
// ─────────────────────────────────────────────────────────────────────────────

pub struct TerminalUI {
    event_tx: Sender<AgentEvent>,
    msg_rx: Receiver<AgentMessage>,
    backend: LinuxBackend,
    renderer: Renderer,
    editor: LineEditor,
    history_lines: Vec<Line>,
    scroll: u16,
    model_name: String,
    input_tokens: u64,
    output_tokens: u64,
    header: HeaderWidget,
    busy: bool,
    spinner: Spinner,
    spinner_start: Option<std::time::Instant>,
    spinner_verb: String,
    response_line_idx: Option<usize>,
    current_response: String,
    /// (line_idx, tool_name, input_summary) — stored when PermissionRequest arrives
    pending_permission: Option<(usize, String, String)>,
    /// Set to true after Ctrl+D sends AgentEvent::Quit. While true, we keep the
    /// UI running and wait for AgentMessage::Evolved so the user sees the
    /// "evolving memories" spinner instead of a silent freeze.
    quitting: bool,
    quitting_line_idx: Option<usize>,
    quitting_start: Option<std::time::Instant>,
}

impl TerminalUI {
    pub fn new(
        event_tx: Sender<AgentEvent>,
        msg_rx: Receiver<AgentMessage>,
    ) -> crate::Result<Self> {
        let mut backend = LinuxBackend::new();
        backend.enter_alt_screen()?;
        backend.enable_raw_mode()?;
        // Switch to a steady (non-blinking) bar cursor via DECSCUSR. Blinking
        // cursors interact badly with streaming redraws — even when we avoid
        // toggling visibility, some terminals re-trigger the blink phase on
        // cursor moves. A steady caret sidesteps the whole class of issues.
        backend.write(b"\x1b[6 q")?;
        backend.flush()?;

        let size = backend.size()?;
        let renderer = Renderer::new(size);

        let header = HeaderWidget::from_env();
        let mut history_lines: Vec<Line> = Vec::new();
        history_lines.push(format_welcome(&header.cwd, header.branch.as_deref()));
        history_lines.push(Line::raw(""));

        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let spinner_verb = random_verb(seed).to_string();

        Ok(TerminalUI {
            event_tx,
            msg_rx,
            backend,
            renderer,
            editor: LineEditor::new(),
            history_lines,
            scroll: 0,
            model_name: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            header,
            busy: false,
            spinner: Spinner::new(),
            spinner_start: None,
            spinner_verb,
            response_line_idx: None,
            current_response: String::new(),
            pending_permission: None,
            quitting: false,
            quitting_line_idx: None,
            quitting_start: None,
        })
    }

    pub fn run(mut self) -> crate::Result<()> {
        let mut event_loop = EventLoop::new()?;
        let mut dirty = true;

        loop {
            // Drain all pending agent messages
            loop {
                match self.msg_rx.try_recv() {
                    Ok(msg) => {
                        let is_evolved = matches!(msg, AgentMessage::Evolved);
                        self.handle_agent_message(msg);
                        dirty = true;
                        if is_evolved {
                            // Agent finished the pre-shutdown evolve step —
                            // safe to tear down the TUI and return.
                            self.cleanup()?;
                            return Ok(());
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        // Agent thread dropped — exit cleanly
                        self.cleanup()?;
                        return Ok(());
                    }
                }
            }

            // Animate spinner while busy and no real response yet
            if self.busy && self.current_response.is_empty() {
                if let Some(idx) = self.response_line_idx {
                    let elapsed = self
                        .spinner_start
                        .map(|s| s.elapsed().as_millis() as u64)
                        .unwrap_or(0);
                    self.history_lines.truncate(idx);
                    self.history_lines.push(Line::from_spans(vec![
                        Span::styled(
                            format!("{} ", self.spinner.frame_at(elapsed)),
                            theme::CLAUDE,
                            false,
                        ),
                        Span::styled(
                            format!("{}\u{2026}", self.spinner_verb),
                            theme::DIM,
                            false,
                        ),
                    ]));
                    dirty = true;
                }
            }

            // Animate the shutdown spinner while waiting for the agent to
            // finish `evolve()` after Ctrl+D.
            if self.quitting {
                if let (Some(start), Some(idx)) =
                    (self.quitting_start, self.quitting_line_idx)
                {
                    let elapsed = start.elapsed().as_millis() as u64;
                    self.history_lines.truncate(idx);
                    self.history_lines.push(Line::from_spans(vec![
                        Span::styled(
                            format!("{} ", self.spinner.frame_at(elapsed)),
                            theme::CLAUDE,
                            false,
                        ),
                        Span::styled(
                            "\u{8fdb}\u{5316}\u{8bb0}\u{5fc6}\u{4e2d}\u{2026} (Ctrl+C \u{5f3a}\u{5236}\u{9000}\u{51fa})"
                                .to_string(),
                            theme::DIM,
                            false,
                        ),
                    ]));
                    dirty = true;
                }
            }

            if dirty {
                // Compute the cursor position before painting, then hand it to
                // the renderer so the final cursor placement is committed in
                // the same synchronized-update block as the diff. This keeps
                // the caret pinned to the input box without toggling cursor
                // visibility, which would otherwise reset the terminal's
                // blink phase on every frame.
                let area = self.renderer.area();
                let input_height = (self.editor.line_count() as u16 + 2).min(8).max(3);
                let chunks = main_layout(input_height).split(area);
                let input_block = Block::new()
                    .border(BorderStyle::Rounded)
                    .borders(BorderSides::HORIZONTAL)
                    .border_fg(theme::DIM);
                let input_inner = input_block.inner(chunks[2]);
                let editor_content = self.editor.content();
                let input_widget =
                    InputWidget::new(&editor_content, self.editor.cursor_offset(), "\u{276F} ")
                        .prompt_fg(theme::CLAUDE);
                let cursor = input_widget.cursor_position(input_inner);

                self.render_frame();
                self.renderer.flush(&mut self.backend, Some(cursor))?;

                dirty = false;
            }

            // Poll keyboard/resize events (~60fps)
            let events = event_loop.poll(16)?;
            for event in events {
                match event {
                    Event::Key(key) => {
                        dirty = true;
                        if self.quitting {
                            // Shutdown in progress — only Ctrl+C force-exits;
                            // every other key is swallowed so the user can't
                            // fire off new input while the agent is evolving.
                            if key == KeyEvent::CtrlC {
                                self.cleanup()?;
                                return Ok(());
                            }
                            continue;
                        }
                        if let Some(action) = self.handle_key(key) {
                            match action {
                                UiAction::Quit => {
                                    let _ = self.event_tx.send(AgentEvent::Quit);
                                    self.enter_quitting_mode();
                                }
                            }
                        }
                    }
                    Event::Resize(new_size) => {
                        self.renderer.resize(new_size);
                        self.scroll =
                            compute_max_scroll(&self.history_lines, &self.renderer, &self.editor);
                        dirty = true;
                    }
                    Event::Tick => {}
                }
            }
        }
    }

    fn handle_agent_message(&mut self, msg: AgentMessage) {
        match msg {
            AgentMessage::Ready { model } => {
                self.model_name = model;
            }

            AgentMessage::Thinking => {
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                self.spinner_verb = random_verb(seed).to_string();
                self.spinner_start = Some(std::time::Instant::now());
                self.current_response = String::new();
                self.busy = true;

                // Push spinner placeholder line
                self.history_lines.push(Line::from_spans(vec![
                    Span::styled(
                        format!("{} ", self.spinner.frame_at(0)),
                        theme::CLAUDE,
                        false,
                    ),
                    Span::styled(
                        format!("{}\u{2026}", self.spinner_verb),
                        theme::DIM,
                        false,
                    ),
                ]));
                self.response_line_idx = Some(self.history_lines.len() - 1);
                self.scroll =
                    compute_max_scroll(&self.history_lines, &self.renderer, &self.editor);
            }

            AgentMessage::TextChunk(s) => {
                self.current_response.push_str(&s);

                if let Some(idx) = self.response_line_idx {
                    // Replace spinner / previous content with the rendered message
                    let rendered = format_assistant_message(&self.current_response);
                    let new_len = idx + rendered.len();
                    self.history_lines.truncate(idx);
                    self.history_lines.extend(rendered);

                    // response_line_idx stays pointing at the first line of the response
                    // but we don't move it; trailing lines grow forward.
                    // Keep scroll at bottom.
                    let _ = new_len; // suppress unused warning
                }
                self.scroll =
                    compute_max_scroll(&self.history_lines, &self.renderer, &self.editor);
            }

            AgentMessage::Status(s) => {
                self.history_lines.push(Line::from_spans(vec![
                    Span::styled(s, theme::DIM, false),
                ]));
                self.scroll =
                    compute_max_scroll(&self.history_lines, &self.renderer, &self.editor);
            }

            AgentMessage::ToolStart { name, input } => {
                let summary = if input.is_empty() {
                    format!("  \u{25b6} {}", name)
                } else {
                    format!("  \u{25b6} {}({})", name, input)
                };
                self.history_lines.push(Line::from_spans(vec![
                    Span::styled(summary, theme::DIM, false),
                ]));
                self.scroll =
                    compute_max_scroll(&self.history_lines, &self.renderer, &self.editor);
            }

            AgentMessage::ToolEnd { .. } => {
                // Output already visible via TextChunk — no-op
            }

            AgentMessage::ToolError { name, error } => {
                let msg = format!("tool error [{}]: {}", name, error);
                self.history_lines.extend(format_error_message(&msg));
                self.scroll =
                    compute_max_scroll(&self.history_lines, &self.renderer, &self.editor);
            }

            AgentMessage::PermissionRequest { tool, input } => {
                self.history_lines.push(render_permission_pending(&tool, &input));
                let idx = self.history_lines.len() - 1;
                self.pending_permission = Some((idx, tool, input));
                self.scroll =
                    compute_max_scroll(&self.history_lines, &self.renderer, &self.editor);
            }

            AgentMessage::Tokens { input, output } => {
                self.input_tokens = input;
                self.output_tokens = output;
            }

            AgentMessage::Done => {
                self.busy = false;
                self.response_line_idx = None;
                self.history_lines.push(Line::raw(""));
                self.scroll =
                    compute_max_scroll(&self.history_lines, &self.renderer, &self.editor);
            }

            AgentMessage::Evolved => {
                // No-op — UI exits via Quit
            }

            AgentMessage::Error(e) => {
                self.history_lines
                    .extend(format_error_message(&format!("error: {}", e)));
                self.busy = false;
                self.scroll =
                    compute_max_scroll(&self.history_lines, &self.renderer, &self.editor);
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<UiAction> {
        // ── Mode 1: Permission pending ──────────────────────────────────────
        if let Some((idx, tool, input)) = self.pending_permission.take() {
            let allowed = match key {
                KeyEvent::Char('y') | KeyEvent::Char('Y') => true,
                KeyEvent::Char('n') | KeyEvent::Char('N') => false,
                _ => {
                    // Not a valid response — put pending_permission back and ignore
                    self.pending_permission = Some((idx, tool, input));
                    return None;
                }
            };
            self.history_lines[idx] = render_permission_result(&tool, &input, allowed);
            self.scroll = compute_max_scroll(&self.history_lines, &self.renderer, &self.editor);
            let _ = self.event_tx.send(AgentEvent::PermissionResponse(allowed));
            return None;
        }

        // ── Mode 2: Busy — Ctrl+C interrupts the agent; every other key
        // falls through to the editor so the user can type (and even queue
        // a submission) while the AI is still streaming its response.
        if self.busy && key == KeyEvent::CtrlC {
            let _ = self.event_tx.send(AgentEvent::Interrupt);
            return None;
        }

        // ── Mode 3: Normal editing (busy or idle) ───────────────────────────
        let action = self.editor.handle_key(key);
        match action {
            EditAction::Submit(line) => {
                if !line.trim().is_empty() {
                    self.history_lines.push(format_user_message(&line));
                    self.scroll =
                        compute_max_scroll(&self.history_lines, &self.renderer, &self.editor);
                    let _ = self.event_tx.send(AgentEvent::Input(line));
                }
            }
            EditAction::Exit => {
                return Some(UiAction::Quit);
            }
            EditAction::Interrupt => {
                self.editor.lines = vec![String::new()];
                self.editor.row = 0;
                self.editor.col = 0;
            }
            EditAction::Continue => {}
        }
        None
    }

    fn render_frame(&mut self) {
        let area = self.renderer.area();
        let input_height = (self.editor.line_count() as u16 + 2).min(8).max(3);
        let chunks = main_layout(input_height).split(area);
        // chunks: [0]=header, [1]=conversation, [2]=input, [3]=status

        let buf = self.renderer.buffer_mut();

        // Header bar
        self.header.render(chunks[0], buf);

        // Conversation history
        let paragraph = Paragraph::new(self.history_lines.clone()).scroll(self.scroll);
        paragraph.render(chunks[1], buf);

        // Input box: top + bottom rounded borders only, dim gray
        let input_block = Block::new()
            .border(BorderStyle::Rounded)
            .borders(BorderSides::HORIZONTAL)
            .border_fg(theme::DIM);
        let input_inner = input_block.inner(chunks[2]);
        input_block.render(chunks[2], buf);

        // Input widget with ❯ prompt (Claude orange)
        let editor_content = self.editor.content();
        let input_widget =
            InputWidget::new(&editor_content, self.editor.cursor_offset(), "\u{276F} ")
                .prompt_fg(theme::CLAUDE);
        input_widget.render(input_inner, buf);

        // Status bar
        let status = StatusWidget {
            model: self.model_name.clone(),
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
        };
        status.render(chunks[3], buf);
    }

    fn cleanup(&mut self) -> crate::Result<()> {
        // Restore the terminal's default cursor style (DECSCUSR reset).
        self.backend.write(b"\x1b[0 q")?;
        self.backend.disable_raw_mode()?;
        self.backend.leave_alt_screen()?;
        self.backend.write(b"Bye!\n")?;
        self.backend.flush()?;
        Ok(())
    }

    /// Enter shutdown state after Ctrl+D: record the spinner anchor and keep
    /// the UI alive so the run loop keeps pumping the message channel until
    /// the agent signals `Evolved`.
    fn enter_quitting_mode(&mut self) {
        self.quitting = true;
        self.quitting_start = Some(std::time::Instant::now());
        self.history_lines.push(Line::raw(""));
        self.quitting_line_idx = Some(self.history_lines.len() - 1);
        self.scroll = compute_max_scroll(&self.history_lines, &self.renderer, &self.editor);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Layout helpers (copied verbatim from repl.rs)
// ─────────────────────────────────────────────────────────────────────────────

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

    let total_width: usize = line
        .spans
        .iter()
        .flat_map(|s| s.text.chars())
        .map(|c| char_width(c) as usize)
        .sum();

    if total_width == 0 {
        return 1; // empty line still takes one row
    }

    total_width.div_ceil(width)
}

// ─────────────────────────────────────────────────────────────────────────────
// LineEditor (copied verbatim from repl.rs)
// ─────────────────────────────────────────────────────────────────────────────

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
            KeyEvent::Home => {
                self.col = 0;
                EditAction::Continue
            }
            KeyEvent::End => {
                self.col = self.lines[self.row].len();
                EditAction::Continue
            }
            KeyEvent::CtrlC => EditAction::Interrupt,
            KeyEvent::CtrlD => {
                if self.is_empty() {
                    EditAction::Exit
                } else {
                    EditAction::Continue
                }
            }
            _ => EditAction::Continue,
        }
    }

    fn prev_char_boundary(&self) -> usize {
        let mut pos = self.col.saturating_sub(1);
        while pos > 0 && !self.lines[self.row].is_char_boundary(pos) {
            pos -= 1;
        }
        pos
    }

    fn next_char_boundary(&self) -> usize {
        let line = &self.lines[self.row];
        let mut pos = self.col + 1;
        while pos < line.len() && !line.is_char_boundary(pos) {
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
