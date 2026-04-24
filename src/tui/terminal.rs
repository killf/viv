use std::sync::mpsc::Receiver;

use crate::agent::protocol::{AgentEvent, AgentMessage};
use crate::core::runtime::channel::NotifySender;
use crate::core::terminal::backend::{Backend, CrossBackend};
use crate::core::terminal::events::{Event, EventLoop};
use crate::core::terminal::input::KeyEvent;
use crate::tui::input::InputMode;
use crate::tui::session::{KeyOutcome, TuiSession};

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
    session: TuiSession,
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
        backend.write(b"\x1b[6 q")?;
        backend.flush()?;

        let size = backend.size()?;
        let (cwd, branch) = Self::read_cwd_branch();
        let host = crate::tui::host::HostInfo::from_env();
        let session = TuiSession::new(size, cwd, branch, host);

        Ok(TerminalUI {
            event_tx,
            msg_rx,
            backend,
            session,
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

            // Animate spinner while busy or quitting
            if self.session.is_busy() || self.session.is_quitting() {
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
                        if self.session.is_quitting() {
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
                                    // handle_key already sent AgentEvent::Quit and entered quitting mode.
                                }
                            }
                        }
                    }
                    Event::Resize(new_size) => {
                        self.session.resize(new_size);
                        dirty = true;
                    }
                    // Tick keeps the loop alive; Mouse events are impossible since we
                    // stopped emitting mouse-tracking sequences.
                    Event::Tick | Event::Mouse(_) => {}
                }
            }
        }
    }

    fn handle_agent_message(&mut self, msg: AgentMessage) -> crate::Result<()> {
        self.session.handle_message(msg, &mut self.backend)
    }

    fn render_frame(&mut self) -> crate::Result<()> {
        let cur = self.session.render_frame(&mut self.backend)?;
        self.backend.move_cursor(cur.row, cur.col)?;
        self.backend.flush()?;
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> crate::Result<Option<UiAction>> {
        match self.session.handle_key(key, &mut self.backend)? {
            KeyOutcome::None => Ok(None),
            KeyOutcome::Event(AgentEvent::Quit) => {
                let _ = self.event_tx.send(AgentEvent::Quit);
                Ok(Some(UiAction::Quit))
            }
            KeyOutcome::Event(e) => {
                let _ = self.event_tx.send(e);
                Ok(None)
            }
        }
    }

    fn cleanup(&mut self) -> crate::Result<()> {
        // Clear the pinned live region so "Bye!" lands cleanly in scrollback.
        let last = self.session.last_live_rows();
        if last > 0 {
            let seq = format!("\x1b[{}A\x1b[0J", last);
            self.backend.write(seq.as_bytes())?;
        }
        // Restore the terminal's default cursor style (DECSCUSR reset).
        self.backend.write(b"\x1b[0 q")?;
        self.backend.disable_raw_mode()?;
        self.backend.write(b"Bye!\r\n")?;
        self.backend.flush()?;
        Ok(())
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
    history: Vec<String>,           // 所有已发送的用户消息
    history_idx: Option<usize>,     // None=当前输入, Some(n)=浏览第 n 条（0=最新）
    original: String,              // 切换历史时保存当前输入

    // 历史搜索状态
    search_query: String,           // 当前搜索字符串
    search_result: Option<String>,  // 搜索结果预览
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
            search_query: String::new(),
            search_result: None,
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
        // 处理历史搜索模式
        if self.mode == InputMode::HistorySearch {
            return self.handle_key_in_search_mode(key);
        }

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
            KeyEvent::CtrlR => {
                // 进入历史搜索模式
                self.original = self.content();
                self.mode = InputMode::HistorySearch;
                self.search_query.clear();
                self.search_result = None;
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
            KeyEvent::CtrlL => {
                // Clear screen: clear the input and redraw
                self.clear();
                EditAction::Continue
            }
            _ => EditAction::Continue,
        }
    }

    /// Handle key events in history search mode (Ctrl+R).
    fn handle_key_in_search_mode(&mut self, key: KeyEvent) -> EditAction {
        match key {
            KeyEvent::Char(ch) => {
                self.search_query.push(ch);
                self.do_history_search();
                EditAction::Continue
            }
            KeyEvent::Backspace => {
                if !self.search_query.is_empty() {
                    self.search_query.pop();
                    self.do_history_search();
                } else {
                    self.exit_history_search();
                }
                EditAction::Continue
            }
            KeyEvent::Escape => {
                self.exit_history_search();
                EditAction::Continue
            }
            KeyEvent::CtrlC => {
                self.exit_history_search();
                EditAction::Interrupt
            }
            KeyEvent::Up => {
                if let Some(idx) = self.history_idx {
                    if idx + 1 < self.history.len() {
                        self.history_idx = Some(idx + 1);
                        self.apply_history_match();
                    }
                } else if !self.history.is_empty() {
                    self.history_idx = Some(0);
                    self.apply_history_match();
                }
                EditAction::Continue
            }
            KeyEvent::Down => {
                if let Some(idx) = self.history_idx {
                    if idx > 0 {
                        self.history_idx = Some(idx - 1);
                        self.apply_history_match();
                    }
                }
                EditAction::Continue
            }
            KeyEvent::Enter => {
                if let Some(ref result) = self.search_result {
                    self.lines = result.lines().map(String::from).collect();
                    if self.lines.is_empty() {
                        self.lines.push(String::new());
                    }
                    self.row = 0;
                    self.col = self.lines[0].len();
                }
                self.mode = InputMode::Chat;
                self.search_query.clear();
                self.search_result = None;
                let content = self.content();
                self.clear();
                EditAction::Submit(content)
            }
            KeyEvent::CtrlR => {
                if let Some(idx) = self.history_idx {
                    if idx + 1 < self.history.len() {
                        self.history_idx = Some(idx + 1);
                        self.apply_history_match();
                    }
                }
                EditAction::Continue
            }
            _ => EditAction::Continue,
        }
    }

    fn do_history_search(&mut self) {
        if self.search_query.is_empty() {
            self.search_result = None;
            self.history_idx = None;
            return;
        }
        for (i, entry) in self.history.iter().enumerate().rev() {
            if entry.contains(&self.search_query) {
                self.search_result = Some(entry.clone());
                self.history_idx = Some(self.history.len() - 1 - i);
                return;
            }
        }
        self.search_result = None;
        self.history_idx = None;
    }

    fn apply_history_match(&mut self) {
        if let Some(ref result) = self.search_result {
            self.lines = result.lines().map(String::from).collect();
            if self.lines.is_empty() {
                self.lines.push(String::new());
            }
            self.row = 0;
            self.col = self.lines[0].len();
        }
    }

    fn exit_history_search(&mut self) {
        self.mode = InputMode::Chat;
        self.lines = vec![self.original.clone()];
        self.row = 0;
        self.col = self.lines[0].len();
        self.search_query.clear();
        self.search_result = None;
        self.history_idx = None;
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
