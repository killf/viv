use std::sync::mpsc::Receiver;

use crate::agent::protocol::{AgentEvent, AgentMessage, PermissionResponse};
use crate::core::runtime::channel::NotifySender;
use crate::core::terminal::backend::{Backend, CrossBackend};
use crate::core::terminal::events::{Event, EventLoop};
use crate::core::terminal::input::KeyEvent;
use crate::tui::content::MarkdownParseBuffer;
use crate::tui::input::InputMode;
use crate::tui::renderer::Renderer;
use crate::tui::spinner::{Spinner, random_verb};

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
    event_tx: NotifySender<AgentEvent>,
    msg_rx: Receiver<AgentMessage>,
    backend: CrossBackend,
    renderer: Renderer,
    live_region: crate::tui::live_region::LiveRegion,
    editor: LineEditor,
    cwd: String,
    branch: Option<String>,
    model_name: String,
    input_tokens: u64,
    output_tokens: u64,
    busy: bool,
    spinner: Spinner,
    spinner_start: Option<std::time::Instant>,
    spinner_verb: String,

    parse_buffer: MarkdownParseBuffer,
    tool_seq: usize,

    /// Permission prompt: (tool_name, input_summary). The menu state lives on
    /// the corresponding `LiveBlock::PermissionPrompt` inside `live_region`.
    pending_permission: Option<(String, String)>,
    /// Set to true after Ctrl+D sends AgentEvent::Quit. While true, we keep the
    /// UI running and wait for AgentMessage::Evolved so the user sees the
    /// "evolving memories" spinner instead of a silent freeze.
    quitting: bool,
    quitting_start: Option<std::time::Instant>,
}

impl TerminalUI {
    /// Read cwd and branch from the environment, without pulling in HeaderWidget.
    fn read_cwd_branch() -> (String, Option<String>) {
        let raw_cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "?".to_string());
        let home = std::env::var("HOME").unwrap_or_default();
        let cwd = if !home.is_empty() && raw_cwd.starts_with(&home) {
            format!("~{}", &raw_cwd[home.len()..])
        } else {
            raw_cwd
        };
        let branch = std::fs::read_to_string(".git/HEAD").ok().and_then(|s| {
            s.trim()
                .strip_prefix("ref: refs/heads/")
                .map(|b| b.to_string())
        });
        // Truncate cwd to 30 chars if needed
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
            cwd
        };
        (cwd, branch)
    }

    pub fn new(
        event_tx: NotifySender<AgentEvent>,
        msg_rx: Receiver<AgentMessage>,
    ) -> crate::Result<Self> {
        let mut backend = CrossBackend::new()?;
        backend.enable_raw_mode()?;
        // Switch to a steady (non-blinking) bar cursor via DECSCUSR. Blinking
        // cursors interact badly with streaming redraws — even when we avoid
        // toggling visibility, some terminals re-trigger the blink phase on
        // cursor moves. A steady caret sidesteps the whole class of issues.
        backend.write(b"\x1b[6 q")?;
        backend.flush()?;

        let size = backend.size()?;
        let renderer = Renderer::new(size);
        let live_region = crate::tui::live_region::LiveRegion::new(size);

        let (cwd, branch) = Self::read_cwd_branch();

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
            live_region,
            editor: LineEditor::new(),
            cwd,
            branch,
            model_name: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            busy: false,
            spinner: Spinner::new(),
            spinner_start: None,
            spinner_verb,
            parse_buffer: MarkdownParseBuffer::new(),
            tool_seq: 0,
            pending_permission: None,
            quitting: false,
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
                        self.handle_agent_message(msg)?;
                        dirty = true;
                        if is_evolved {
                            // Agent finished the pre-shutdown evolve step --
                            // safe to tear down the TUI and return.
                            self.cleanup()?;
                            return Ok(());
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        // Agent thread dropped -- exit cleanly
                        self.cleanup()?;
                        return Ok(());
                    }
                }
            }

            // Animate spinner while busy
            if self.busy && self.spinner_start.is_some() {
                dirty = true;
            }

            // Animate the shutdown spinner while waiting for the agent to
            // finish `evolve()` after Ctrl+D.
            if self.quitting && self.quitting_start.is_some() {
                dirty = true;
            }

            if dirty {
                self.render_frame()?;
                dirty = false;
            }

            // Poll keyboard/resize events (~60fps)
            let events = event_loop.poll(16)?;
            for event in events {
                match event {
                    Event::Key(key) => {
                        dirty = true;
                        if self.quitting {
                            // Shutdown in progress -- only Ctrl+C force-exits;
                            // every other key is swallowed so the user can't
                            // fire off new input while the agent is evolving.
                            if key == KeyEvent::CtrlC {
                                self.cleanup()?;
                                return Ok(());
                            }
                            continue;
                        }
                        if let Some(action) = self.handle_key(key)? {
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
                        self.live_region.resize(new_size);
                        dirty = true;
                    }
                    Event::Tick => {}
                    Event::Mouse(_) => {}
                }
            }
        }
    }

    fn handle_agent_message(&mut self, msg: AgentMessage) -> crate::Result<()> {
        match msg {
            AgentMessage::Ready { model } => {
                self.model_name = model.clone();
                let welcome_widget = crate::tui::welcome::WelcomeWidget::new(
                    Some(&model),
                    &self.cwd,
                    self.branch.as_deref(),
                );
                let width = self.renderer.area().width;
                let welcome_text = welcome_widget.as_scrollback_string(width);
                self.backend.write(welcome_text.as_bytes())?;
                self.backend.flush()?;
            }

            AgentMessage::Thinking => {
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                self.spinner_verb = random_verb(seed).to_string();
                self.spinner_start = Some(std::time::Instant::now());
                self.busy = true;
            }

            AgentMessage::TextChunk(s) => {
                let new_blocks = self.parse_buffer.push(&s);
                for block in new_blocks {
                    if let crate::tui::content::ContentBlock::Markdown { nodes } = block {
                        self.live_region.push_live_block(
                            crate::tui::live_region::LiveBlock::Markdown {
                                nodes,
                                state: crate::tui::live_region::BlockState::Committing,
                            },
                        );
                    }
                }
                let pending = self.parse_buffer.peek_pending();
                self.live_region.drop_trailing_live_markdown();
                if !pending.is_empty() {
                    self.live_region.push_live_block(
                        crate::tui::live_region::LiveBlock::Markdown {
                            nodes: pending,
                            state: crate::tui::live_region::BlockState::Live,
                        },
                    );
                }
            }

            AgentMessage::Status(s) => {
                self.live_region.commit_text(&mut self.backend, &s)?;
            }

            AgentMessage::ToolStart { name, input } => {
                let id = self.tool_seq;
                self.tool_seq += 1;
                self.live_region.push_live_block(
                    crate::tui::live_region::LiveBlock::ToolCall {
                        id,
                        name,
                        input,
                        output: None,
                        error: None,
                        tc_state: crate::tui::tool_call::ToolCallState::new_running(),
                        state: crate::tui::live_region::BlockState::Live,
                    },
                );
            }

            AgentMessage::ToolEnd { name: _, output } => {
                self.live_region.finish_last_running_tool(Some(output), None);
            }

            AgentMessage::ToolError { name: _, error } => {
                self.live_region.finish_last_running_tool(None, Some(error));
            }

            AgentMessage::PermissionRequest { tool, input } => {
                self.pending_permission = Some((tool.clone(), input.clone()));
                self.live_region.push_live_block(
                    crate::tui::live_region::LiveBlock::PermissionPrompt {
                        tool,
                        input,
                        menu: crate::tui::permission::PermissionState::new(),
                    },
                );
            }

            AgentMessage::Tokens { input, output } => {
                self.input_tokens = input;
                self.output_tokens = output;
            }

            AgentMessage::Done => {
                // Flush remaining parse buffer
                let remaining = self.parse_buffer.flush();
                for block in remaining {
                    if let crate::tui::content::ContentBlock::Markdown { nodes } = block {
                        self.live_region.push_live_block(
                            crate::tui::live_region::LiveBlock::Markdown {
                                nodes,
                                state: crate::tui::live_region::BlockState::Committing,
                            },
                        );
                    }
                }
                self.live_region.drop_trailing_live_markdown();
                self.busy = false;
                self.spinner_start = None;
            }

            AgentMessage::Evolved => {
                // No-op -- UI exits via Quit
            }

            AgentMessage::Error(e) => {
                let msg = format!("\u{25cf} error: {}", e);
                self.live_region.commit_text(&mut self.backend, &msg)?;
                self.busy = false;
                self.spinner_start = None;
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> crate::Result<Option<UiAction>> {
        // ── Mode 1: Permission pending ──────────────────────────────────────
        if self.pending_permission.is_some() {
            match key {
                KeyEvent::Up => {
                    if let Some(menu) = self.live_region.permission_menu_mut() {
                        menu.move_up();
                    }
                    return Ok(None);
                }
                KeyEvent::Down => {
                    if let Some(menu) = self.live_region.permission_menu_mut() {
                        menu.move_down();
                    }
                    return Ok(None);
                }
                KeyEvent::Enter => {
                    // Capture the selected option from the live region's menu
                    let selected_opt = self
                        .live_region
                        .permission_menu()
                        .map(|m| m.selected_option());
                    let selected = match selected_opt {
                        Some(s) => s,
                        None => {
                            self.pending_permission = None;
                            return Ok(None);
                        }
                    };
                    let (tool, input) = match self.pending_permission.take() {
                        Some(t) => t,
                        None => return Ok(None),
                    };
                    let response = match selected {
                        crate::tui::permission::PermissionOption::Deny => PermissionResponse::Deny,
                        crate::tui::permission::PermissionOption::Allow => {
                            PermissionResponse::Allow
                        }
                        crate::tui::permission::PermissionOption::AlwaysAllow => {
                            PermissionResponse::AlwaysAllow
                        }
                    };
                    let result_text = match selected {
                        crate::tui::permission::PermissionOption::Deny => {
                            format!(
                                "  \u{2717} {}  {} ({})",
                                selected.short_label(),
                                tool,
                                input
                            )
                        }
                        _ => {
                            format!(
                                "  \u{2713} {}  {} ({})",
                                selected.short_label(),
                                tool,
                                input
                            )
                        }
                    };
                    self.live_region.drop_permission_prompt();
                    self.live_region.commit_text(&mut self.backend, &result_text)?;
                    let _ = self.event_tx.send(AgentEvent::PermissionResponse(response));
                    return Ok(None);
                }
                _ => {
                    // All other keys are swallowed while permission is pending
                    return Ok(None);
                }
            }
        }

        // ── Mode 3: Busy -- Ctrl+C interrupts the agent; every other key
        // falls through to the editor so the user can type (and even queue
        // a submission) while the AI is still streaming its response.
        if key == KeyEvent::CtrlC && self.busy {
            let _ = self.event_tx.send(AgentEvent::Interrupt);
            return Ok(None);
        }

        // ── Mode 4: Normal editing (busy or idle) ───────────────────────────
        let mode = self.editor.mode;
        let action = self.editor.handle_key(key);
        match action {
            EditAction::Submit(line) => {
                if !line.trim().is_empty() {
                    // Slash/colon commands are not added to the conversation history
                    let is_command = mode != InputMode::Chat;
                    if !is_command {
                        let text = format!("> {}", line);
                        self.live_region.commit_text(&mut self.backend, &text)?;
                        self.editor.push_history(line.clone());
                    }
                    let event = match mode {
                        InputMode::SlashCommand => AgentEvent::SlashCommand(line),
                        InputMode::ColonCommand => AgentEvent::ColonCommand(line),
                        InputMode::Chat => AgentEvent::Input(line),
                    };
                    let _ = self.event_tx.send(event);
                }
            }
            EditAction::Exit => {
                return Ok(Some(UiAction::Quit));
            }
            EditAction::Interrupt => {
                self.editor.clear();
            }
            EditAction::Continue => {}
        }
        Ok(None)
    }

    fn render_frame(&mut self) -> crate::Result<()> {
        let spinner_frame = if (self.busy && self.spinner_start.is_some()) || self.quitting {
            let elapsed = self
                .spinner_start
                .or(self.quitting_start)
                .map(|s| s.elapsed().as_millis() as u64)
                .unwrap_or(0);
            self.spinner.frame_at(elapsed).chars().next()
        } else {
            None
        };
        let ctx = crate::tui::status::StatusContext {
            cwd: self.cwd.clone(),
            branch: self.branch.clone(),
            model: self.model_name.clone(),
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            spinner_frame,
            spinner_verb: self.spinner_verb.clone(),
        };
        let editor = self.editor.content();
        let offset = self.editor.cursor_offset();
        let mode = self.editor.mode;
        let cur = self
            .live_region
            .frame(&mut self.backend, &editor, offset, mode, &ctx)?;
        self.backend.move_cursor(cur.row, cur.col)?;
        self.backend.flush()?;
        Ok(())
    }

    fn cleanup(&mut self) -> crate::Result<()> {
        // Clear the pinned live region so "Bye!" lands cleanly in scrollback.
        let last = self.live_region.last_live_rows();
        if last > 0 {
            let seq = format!("\x1b[{}A\x1b[0J", last);
            self.backend.write(seq.as_bytes())?;
        }
        // Restore the terminal's default cursor style (DECSCUSR reset).
        self.backend.write(b"\x1b[0 q")?;
        self.backend.disable_raw_mode()?;
        self.backend.write(b"Bye!\n")?;
        self.backend.flush()?;
        Ok(())
    }

    /// Enter shutdown state after Ctrl+D: record the spinner start and keep
    /// the UI alive so the run loop keeps pumping the message channel until
    /// the agent signals `Evolved`.
    fn enter_quitting_mode(&mut self) {
        self.quitting = true;
        self.quitting_start = Some(std::time::Instant::now());
    }
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
    pub mode: InputMode,
    history: Vec<String>,       // 所有已发送的用户消息
    history_idx: Option<usize>, // None=当前输入, Some(n)=浏览第 n 条（0=最新）
    original: String,           // 切换历史时保存当前输入
}

impl LineEditor {
    pub fn new() -> Self {
        LineEditor {
            lines: vec![String::new()],
            row: 0,
            col: 0,
            mode: InputMode::Chat,
            history: Vec::new(),
            history_idx: None,
            original: String::new(),
        }
    }

    /// Push a submitted line into history.
    pub fn push_history(&mut self, line: String) {
        if !line.trim().is_empty() {
            self.history.push(line);
        }
        self.history_idx = None;
        self.original.clear();
    }

    /// Restore the original input and exit history browsing mode.
    fn exit_history(&mut self) {
        if self.history_idx.is_some() {
            self.lines = vec![self.original.clone()];
            self.row = 0;
            self.col = self.lines[0].len();
            self.history_idx = None;
        }
    }

    /// Clear the editor buffer and reset mode to Chat.
    pub fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.row = 0;
        self.col = 0;
        self.mode = InputMode::Chat;
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
                self.exit_history();
                // Detect mode change: when first char of empty line is `/` or `:`
                if self.lines[self.row].is_empty() && self.col == 0 {
                    match ch {
                        '/' => self.mode = InputMode::SlashCommand,
                        ':' => self.mode = InputMode::ColonCommand,
                        _ => {}
                    }
                }
                self.lines[self.row].insert(self.col, ch);
                self.col += ch.len_utf8();
                EditAction::Continue
            }
            KeyEvent::ShiftEnter => {
                self.exit_history();
                let rest = self.lines[self.row].split_off(self.col);
                self.lines.insert(self.row + 1, rest);
                self.row += 1;
                self.col = 0;
                EditAction::Continue
            }
            KeyEvent::Enter => {
                let content = self.content();
                self.clear();
                EditAction::Submit(content)
            }
            KeyEvent::Backspace => {
                self.exit_history();
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
                self.exit_history();
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
                self.exit_history();
                if self.col > 0 {
                    self.col = self.prev_char_boundary();
                } else if self.row > 0 {
                    self.row -= 1;
                    self.col = self.lines[self.row].len();
                }
                EditAction::Continue
            }
            KeyEvent::Right => {
                self.exit_history();
                if self.col < self.lines[self.row].len() {
                    self.col = self.next_char_boundary();
                } else if self.row + 1 < self.lines.len() {
                    self.row += 1;
                    self.col = 0;
                }
                EditAction::Continue
            }
            KeyEvent::Up => {
                // History browsing: ↑ moves to older entries
                if self.history.is_empty() {
                    return EditAction::Continue;
                }
                match self.history_idx {
                    None => {
                        // First ↑: save current input, jump to most recent entry
                        self.original = self.content();
                        self.history_idx = Some(0);
                    }
                    Some(idx) => {
                        if idx + 1 >= self.history.len() {
                            // Already at the oldest entry, stay
                            return EditAction::Continue;
                        }
                        self.history_idx = Some(idx + 1);
                    }
                }
                let idx = self.history_idx.unwrap();
                let entry = &self.history[self.history.len() - 1 - idx];
                self.lines = entry.lines().map(String::from).collect();
                if self.lines.is_empty() {
                    self.lines.push(String::new());
                }
                self.row = 0;
                self.col = self.lines[0].len();
                EditAction::Continue
            }
            KeyEvent::Down => {
                // History browsing: ↓ moves to newer entries / restores original
                match self.history_idx {
                    None => EditAction::Continue,
                    Some(0) => {
                        // At newest entry: restore original input
                        self.lines = vec![self.original.clone()];
                        self.row = 0;
                        self.col = self.lines[0].len();
                        self.history_idx = None;
                        EditAction::Continue
                    }
                    Some(idx) => {
                        self.history_idx = Some(idx - 1);
                        let entry_idx = self.history.len() - 1 - idx;
                        let entry = &self.history[entry_idx];
                        self.lines = entry.lines().map(String::from).collect();
                        if self.lines.is_empty() {
                            self.lines.push(String::new());
                        }
                        self.row = 0;
                        self.col = self.lines[0].len();
                        EditAction::Continue
                    }
                }
            }
            KeyEvent::Home => {
                self.exit_history();
                self.col = 0;
                EditAction::Continue
            }
            KeyEvent::End => {
                self.exit_history();
                self.col = self.lines[self.row].len();
                EditAction::Continue
            }
            KeyEvent::CtrlC => {
                self.clear();
                EditAction::Interrupt
            }
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
