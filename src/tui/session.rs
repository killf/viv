//! Shared TUI rendering core.
//!
//! `TuiSession` owns all state that drives the input box / live region /
//! status bar rendering. It takes `&mut dyn Backend` on every method so
//! the same code can drive a real terminal (CrossBackend) and a test
//! simulator (TestBackend).

use std::time::Instant;

use crate::agent::protocol::{AgentEvent, AgentMessage, PermissionResponse};
use crate::core::terminal::backend::Backend;
use crate::core::terminal::input::KeyEvent;
use crate::core::terminal::size::TermSize;
use crate::tui::content::MarkdownParseBuffer;
use crate::tui::input::InputMode;
use crate::tui::live_region::{BlockState, CursorPos, LiveBlock, LiveRegion};
use crate::tui::spinner::{Spinner, random_verb};
use crate::tui::status::StatusContext;
pub use crate::tui::terminal::{EditAction, LineEditor};

/// Outcome of a keypress handled by [`TuiSession::handle_key`].
pub enum KeyOutcome {
    /// Nothing to propagate to the agent; state already updated.
    None,
    /// Forward this event to the agent (Submit, Interrupt, PermissionResponse, Quit).
    Event(AgentEvent),
}

pub struct TuiSession {
    // Core rendering state.
    live_region: LiveRegion,
    editor: LineEditor,

    // Display context (shown in status bar / welcome header).
    cwd: String,
    branch: Option<String>,
    model_name: String,

    // Stats.
    input_tokens: u64,
    output_tokens: u64,

    // Busy / spinner.
    busy: bool,
    spinner: Spinner,
    spinner_start: Option<Instant>,
    spinner_verb: String,

    // Parsing.
    parse_buffer: MarkdownParseBuffer,
    tool_seq: usize,

    // Permission flow.
    pending_permission: Option<(String, String)>,

    // Shutdown.
    quitting: bool,
    quitting_start: Option<Instant>,
}

impl TuiSession {
    pub fn new(size: TermSize, cwd: String, branch: Option<String>) -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let spinner_verb = random_verb(seed).to_string();
        TuiSession {
            live_region: LiveRegion::new(size),
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
        }
    }

    pub fn resize(&mut self, new_size: TermSize) {
        self.live_region.resize(new_size);
    }

    pub fn is_busy(&self) -> bool {
        self.busy
    }

    pub fn is_quitting(&self) -> bool {
        self.quitting
    }

    pub fn last_live_rows(&self) -> u16 {
        self.live_region.last_live_rows()
    }

    pub fn input_content(&self) -> String {
        self.editor.content()
    }

    pub fn input_mode(&self) -> InputMode {
        self.editor.mode
    }

    pub fn permission_selected(&self) -> Option<usize> {
        self.live_region.permission_menu().map(|m| m.selected)
    }

    pub fn enter_quitting_mode(&mut self) {
        self.quitting = true;
        self.quitting_start = Some(Instant::now());
    }

    pub fn handle_message(
        &mut self,
        msg: AgentMessage,
        backend: &mut dyn Backend,
    ) -> crate::Result<()> {
        match msg {
            AgentMessage::Ready { model } => {
                self.model_name = model.clone();
                let welcome_widget = crate::tui::welcome::WelcomeWidget::new(
                    Some(&model),
                    &self.cwd,
                    self.branch.as_deref(),
                );
                let width = self.live_region.width();
                let welcome_text = welcome_widget.as_scrollback_string(width);
                backend.write(welcome_text.as_bytes())?;
                backend.flush()?;
            }
            AgentMessage::Thinking => {
                let seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                self.spinner_verb = random_verb(seed).to_string();
                self.spinner_start = Some(Instant::now());
                self.busy = true;
            }
            AgentMessage::TextChunk(s) => {
                let new_blocks = self.parse_buffer.push(&s);
                for block in new_blocks {
                    if let crate::tui::content::ContentBlock::Markdown { nodes } = block {
                        self.live_region.push_live_block(LiveBlock::Markdown {
                            nodes,
                            state: BlockState::Committing,
                        });
                    }
                }
                let pending = self.parse_buffer.peek_pending();
                self.live_region.drop_trailing_live_markdown();
                if !pending.is_empty() {
                    self.live_region.push_live_block(LiveBlock::Markdown {
                        nodes: pending,
                        state: BlockState::Live,
                    });
                }
            }
            AgentMessage::Status(s) => {
                self.live_region.commit_text(backend, &s)?;
            }
            AgentMessage::ToolStart { name, input } => {
                let id = self.tool_seq;
                self.tool_seq += 1;
                self.live_region.push_live_block(LiveBlock::ToolCall {
                    id,
                    name,
                    input,
                    output: None,
                    error: None,
                    tc_state: crate::tui::tool_call::ToolCallState::new_running(),
                    state: BlockState::Live,
                });
            }
            AgentMessage::ToolEnd { name: _, output } => {
                self.live_region.finish_last_running_tool(Some(output), None);
            }
            AgentMessage::ToolError { name: _, error } => {
                self.live_region.finish_last_running_tool(None, Some(error));
            }
            AgentMessage::PermissionRequest { tool, input } => {
                self.pending_permission = Some((tool.clone(), input.clone()));
                self.live_region.push_live_block(LiveBlock::PermissionPrompt {
                    tool,
                    input,
                    menu: crate::tui::permission::PermissionState::new(),
                });
            }
            AgentMessage::Tokens { input, output } => {
                self.input_tokens = input;
                self.output_tokens = output;
            }
            AgentMessage::Done => {
                let remaining = self.parse_buffer.flush();
                for block in remaining {
                    if let crate::tui::content::ContentBlock::Markdown { nodes } = block {
                        self.live_region.push_live_block(LiveBlock::Markdown {
                            nodes,
                            state: BlockState::Committing,
                        });
                    }
                }
                self.live_region.drop_trailing_live_markdown();
                self.busy = false;
                self.spinner_start = None;
            }
            AgentMessage::Evolved => {}
            AgentMessage::Error(e) => {
                let msg = format!("\u{25cf} error: {}", e);
                self.live_region.commit_text(backend, &msg)?;
                self.busy = false;
                self.spinner_start = None;
            }
        }
        Ok(())
    }

    pub fn render_frame(&mut self, backend: &mut dyn Backend) -> crate::Result<CursorPos> {
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
        let ctx = StatusContext {
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
        self.live_region.frame(backend, &editor, offset, mode, &ctx)
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        backend: &mut dyn Backend,
    ) -> crate::Result<KeyOutcome> {
        // ── Permission pending ───────────────────────────────────────
        if self.pending_permission.is_some() {
            match key {
                KeyEvent::Up => {
                    if let Some(menu) = self.live_region.permission_menu_mut() {
                        menu.move_up();
                    }
                    return Ok(KeyOutcome::None);
                }
                KeyEvent::Down => {
                    if let Some(menu) = self.live_region.permission_menu_mut() {
                        menu.move_down();
                    }
                    return Ok(KeyOutcome::None);
                }
                KeyEvent::Enter => {
                    let selected_opt = self
                        .live_region
                        .permission_menu()
                        .map(|m| m.selected_option());
                    let selected = match selected_opt {
                        Some(s) => s,
                        None => {
                            self.pending_permission = None;
                            return Ok(KeyOutcome::None);
                        }
                    };
                    let (tool, input) = match self.pending_permission.take() {
                        Some(t) => t,
                        None => return Ok(KeyOutcome::None),
                    };
                    let response = match selected {
                        crate::tui::permission::PermissionOption::Deny => {
                            PermissionResponse::Deny
                        }
                        crate::tui::permission::PermissionOption::Allow => {
                            PermissionResponse::Allow
                        }
                        crate::tui::permission::PermissionOption::AlwaysAllow => {
                            PermissionResponse::AlwaysAllow
                        }
                    };
                    let result_text = match selected {
                        crate::tui::permission::PermissionOption::Deny => format!(
                            "  \u{2717} {}  {} ({})",
                            selected.short_label(),
                            tool,
                            input
                        ),
                        _ => format!(
                            "  \u{2713} {}  {} ({})",
                            selected.short_label(),
                            tool,
                            input
                        ),
                    };
                    self.live_region.drop_permission_prompt();
                    self.live_region.commit_text(backend, &result_text)?;
                    return Ok(KeyOutcome::Event(AgentEvent::PermissionResponse(response)));
                }
                _ => return Ok(KeyOutcome::None),
            }
        }

        // ── Ctrl+C while busy: interrupt agent ───────────────────────
        if key == KeyEvent::CtrlC && self.busy {
            return Ok(KeyOutcome::Event(AgentEvent::Interrupt));
        }

        // ── Normal editing ───────────────────────────────────────────
        let mode = self.editor.mode;
        let action = self.editor.handle_key(key);
        match action {
            EditAction::Submit(line) => {
                if !line.trim().is_empty() {
                    let is_command = mode != InputMode::Chat;
                    if !is_command {
                        let text = format!("> {}", line);
                        self.live_region.commit_text(backend, &text)?;
                        self.editor.push_history(line.clone());
                    }
                    let event = match mode {
                        InputMode::SlashCommand => AgentEvent::SlashCommand(line),
                        InputMode::ColonCommand => AgentEvent::ColonCommand(line),
                        InputMode::Chat | InputMode::HistorySearch => AgentEvent::Input(line),
                    };
                    return Ok(KeyOutcome::Event(event));
                }
                Ok(KeyOutcome::None)
            }
            EditAction::Exit => {
                self.enter_quitting_mode();
                Ok(KeyOutcome::Event(AgentEvent::Quit))
            }
            EditAction::Interrupt => {
                self.editor.clear();
                Ok(KeyOutcome::None)
            }
            EditAction::Continue => Ok(KeyOutcome::None),
        }
    }
}
