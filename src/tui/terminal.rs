use std::sync::mpsc::Receiver;

use crate::agent::protocol::{AgentEvent, AgentMessage, PermissionResponse};
use crate::core::runtime::channel::NotifySender;
use crate::core::terminal::backend::{Backend, CrossBackend};
use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::events::{Event, EventLoop};
use crate::core::terminal::input::{KeyEvent, MouseEvent};
use crate::core::terminal::style::theme;
use crate::tui::block::{Block, BorderSides, BorderStyle};
use crate::tui::code_block::CodeBlockWidget;
use crate::tui::content::{ContentBlock, MarkdownNode, MarkdownParseBuffer};
use crate::tui::conversation::ConversationState;
use crate::tui::focus::FocusManager;
use crate::tui::input::{InputMode, InputWidget};
use crate::tui::layout::{Constraint, Direction, Layout};
use crate::tui::markdown::MarkdownBlockWidget;
use crate::tui::permission::{PermissionState, PermissionWidget};
use crate::tui::renderer::Renderer;
use crate::tui::selection::SelectionState;
use crate::tui::spinner::{random_verb, Spinner};
use crate::tui::status::StatusWidget;
use crate::tui::text_map::{CellSource, TextMap};
use crate::tui::tool_call::{extract_input_summary, ToolCallState, ToolCallWidget, ToolStatus};
use crate::tui::welcome::WelcomeWidget;
use crate::tui::widget::{StatefulWidget, Widget};

// ─────────────────────────────────────────────────────────────────────────────
// Welcome animation state
// ─────────────────────────────────────────────────────────────────────────────

/// Tracks the start time of the welcome screen fade-in animation.
struct WelcomeAnimState {
    start: std::time::Instant,
    /// Total rows (logo + info), used to compute the animation end time.
    total_rows: u16,
}

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

    // ── Widget-based conversation model ─────────────────────────────────
    blocks: Vec<ContentBlock>,
    parse_buffer: MarkdownParseBuffer,
    conversation_state: ConversationState,
    tool_states: Vec<ToolCallState>,
    focus: FocusManager,
    tool_seq: usize,

    /// Permission state: (block_idx, tool_name, input_summary, menu_state).
    /// When Some, the permission options menu is shown in the input area.
    pending_permission: Option<(usize, String, String, PermissionState)>,
    /// Set to true after Ctrl+D sends AgentEvent::Quit. While true, we keep the
    /// UI running and wait for AgentMessage::Evolved so the user sees the
    /// "evolving memories" spinner instead of a silent freeze.
    quitting: bool,
    quitting_start: Option<std::time::Instant>,
    /// Welcome screen fade-in animation state. None means animation is complete.
    welcome_anim: Option<WelcomeAnimState>,
    /// Mouse drag-to-select state.
    selection_state: SelectionState,
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

        let mut blocks = Vec::new();
        let mut conversation_state = ConversationState::new();

        // Push welcome screen as first content block
        blocks.push(ContentBlock::Welcome {
            model: None,
            cwd: cwd.clone(),
            branch: branch.clone(),
        });
        conversation_state.append_item_height(5); // WelcomeWidget::HEIGHT

        // Empty line separator
        blocks.push(ContentBlock::Markdown {
            nodes: vec![MarkdownNode::Paragraph {
                spans: vec![crate::tui::content::InlineSpan::Text(String::new())],
            }],
        });
        conversation_state.append_item_height(1);

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
            blocks,
            parse_buffer: MarkdownParseBuffer::new(),
            conversation_state,
            tool_states: Vec::new(),
            focus: FocusManager::new(),
            tool_seq: 0,
            pending_permission: None,
            quitting: false,
            quitting_start: None,
            welcome_anim: Some(WelcomeAnimState {
                start: std::time::Instant::now(),
                total_rows: WelcomeWidget::TOTAL_ROWS,
            }),
            selection_state: SelectionState::new(),
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
                // Compute the cursor position before painting.
                let area = self.renderer.area();
                let is_permission_pending = self.pending_permission.is_some();
                let input_height = if is_permission_pending {
                    PermissionWidget::height()
                } else {
                    (self.editor.line_count() as u16 + 2).clamp(3, 8)
                };
                let chunks = main_layout(input_height).split(area);
                let input_block_area = chunks[1];

                let cursor = if is_permission_pending {
                    // No cursor in permission mode
                    None
                } else {
                    let input_block = Block::new()
                        .border(BorderStyle::Rounded)
                        .borders(BorderSides::HORIZONTAL)
                        .border_fg(theme::DIM);
                    let input_inner = input_block.inner(input_block_area);
                    let editor_content = self.editor.content();
                    let input_widget = InputWidget::new(
                        &editor_content,
                        self.editor.cursor_offset(),
                        self.editor.mode.prompt(),
                    )
                    .prompt_fg(theme::CLAUDE);
                    Some(input_widget.cursor_position(input_inner))
                };

                self.render_frame();
                self.renderer
                    .set_selection(self.selection_state.region().map(|r| r.as_rect()));
                self.renderer.flush(&mut self.backend, cursor)?;

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
                        self.selection_state.clear(); // coordinates invalid after resize
                        self.renderer.resize(new_size);
                        // Recalculate all block heights (width changed -> word wrap changes)
                        let width = new_size.cols;
                        for (i, block) in self.blocks.iter().enumerate() {
                            let h = block_height_with_width(block, width);
                            self.conversation_state.set_item_height(i, h);
                        }
                        self.conversation_state.auto_scroll();
                        dirty = true;
                    }
                    Event::Tick => {}
                    Event::Mouse(MouseEvent::LeftPress { x, y }) => {
                        // Only handle clicks in the conversation area (between header and status bar)
                        let header_height: u16 = 3;
                        let status_height: u16 = 1;
                        let area = self.renderer.area();
                        let conv_top = header_height;
                        let conv_bottom = area.height.saturating_sub(status_height);

                        if y >= conv_top && y <= conv_bottom {
                            self.selection_state.start_drag(x, y);
                            dirty = true;
                        }
                    }
                    Event::Mouse(MouseEvent::LeftDrag { x, y }) => {
                        if self.selection_state.is_dragging() {
                            self.selection_state.update_drag(x, y);
                            dirty = true;
                        }
                    }
                    Event::Mouse(MouseEvent::LeftRelease { x, y }) => {
                        if self.selection_state.is_dragging() {
                            self.selection_state.end_drag(x, y);
                            dirty = true;
                        }
                    }
                }
            }
        }
    }

    fn handle_agent_message(&mut self, msg: AgentMessage) {
        match msg {
            AgentMessage::Ready { model } => {
                self.model_name = model.clone();
                // Update the Welcome block's model field
                if let Some(ContentBlock::Welcome { model: m, .. }) = self.blocks.first_mut() {
                    *m = Some(model);
                }
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
                    let h = self.block_height(&block);
                    self.blocks.push(block);
                    self.conversation_state.append_item_height(h);
                }
                // If no new blocks were emitted but content is growing in the
                // parse buffer, update the last markdown block's height if it
                // was recently appended and the parse buffer produced an update.
                self.conversation_state.auto_scroll();
            }

            AgentMessage::Status(s) => {
                // Render status as a dim markdown paragraph
                let nodes = vec![MarkdownNode::Paragraph {
                    spans: vec![crate::tui::content::InlineSpan::Text(s)],
                }];
                self.blocks.push(ContentBlock::Markdown { nodes });
                self.conversation_state.append_item_height(1);
                self.conversation_state.auto_scroll();
            }

            AgentMessage::ToolStart { name, input } => {
                let _id = self.tool_seq;
                self.tool_seq += 1;
                self.blocks.push(ContentBlock::ToolCall {
                    id: _id,
                    name,
                    input,
                    output: None,
                    error: None,
                });
                self.tool_states.push(ToolCallState::new_running());
                self.conversation_state.append_item_height(1); // folded = 1 line
                self.conversation_state.auto_scroll();
            }

            AgentMessage::ToolEnd { name: _, output } => {
                // Find the most recent Running tool call (reverse search)
                if let Some(idx) = self
                    .tool_states
                    .iter()
                    .rposition(|s| matches!(s.status, ToolStatus::Running))
                {
                    let summary = format!("{} chars", output.len());
                    self.tool_states[idx] = ToolCallState::new_success(summary);
                    // Update the ContentBlock's output field
                    let mut tc_idx = 0;
                    for block in &mut self.blocks {
                        if let ContentBlock::ToolCall { output: o, .. } = block {
                            if tc_idx == idx {
                                *o = Some(output);
                                break;
                            }
                            tc_idx += 1;
                        }
                    }
                }
            }

            AgentMessage::ToolError { name: _, error } => {
                // Find the most recent Running tool call (reverse search)
                if let Some(idx) = self
                    .tool_states
                    .iter()
                    .rposition(|s| matches!(s.status, ToolStatus::Running))
                {
                    let msg = if error.len() > 60 {
                        format!("{}...", &error[..60])
                    } else {
                        error.clone()
                    };
                    self.tool_states[idx] = ToolCallState::new_error(msg);
                    // Update the ContentBlock's error field
                    let mut tc_idx = 0;
                    for block in &mut self.blocks {
                        if let ContentBlock::ToolCall { error: e, .. } = block {
                            if tc_idx == idx {
                                *e = Some(error);
                                break;
                            }
                            tc_idx += 1;
                        }
                    }
                }
            }

            AgentMessage::PermissionRequest { tool, input } => {
                // Store the permission prompt as a Markdown block in conversation
                let prompt_text = format!("  \u{25c6} {}({})", tool, input);
                let nodes = vec![MarkdownNode::Paragraph {
                    spans: vec![crate::tui::content::InlineSpan::Text(prompt_text)],
                }];
                self.blocks.push(ContentBlock::Markdown { nodes });
                let idx = self.blocks.len() - 1;
                self.conversation_state.append_item_height(1);
                self.pending_permission = Some((idx, tool, input, PermissionState::new()));
                self.conversation_state.auto_scroll();
            }

            AgentMessage::Tokens { input, output } => {
                self.input_tokens = input;
                self.output_tokens = output;
            }

            AgentMessage::Done => {
                // Flush remaining parse buffer
                let remaining = self.parse_buffer.flush();
                for block in remaining {
                    let h = self.block_height(&block);
                    self.blocks.push(block);
                    self.conversation_state.append_item_height(h);
                }
                self.busy = false;
                self.spinner_start = None;

                // Empty line separator
                let nodes = vec![MarkdownNode::Paragraph {
                    spans: vec![crate::tui::content::InlineSpan::Text(String::new())],
                }];
                self.blocks.push(ContentBlock::Markdown { nodes });
                self.conversation_state.append_item_height(1);
                self.conversation_state.auto_scroll();
            }

            AgentMessage::Evolved => {
                // No-op -- UI exits via Quit
            }

            AgentMessage::Error(e) => {
                let msg = format!("\u{25cf} error: {}", e);
                let nodes = vec![MarkdownNode::Paragraph {
                    spans: vec![crate::tui::content::InlineSpan::Text(msg)],
                }];
                self.blocks.push(ContentBlock::Markdown { nodes });
                self.conversation_state.append_item_height(1);
                self.busy = false;
                self.spinner_start = None;
                self.conversation_state.auto_scroll();
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<UiAction> {
        // ── Global scroll: Ctrl+K / Ctrl+J ───────────────────────────────────
        match key {
            KeyEvent::CtrlChar('k') => {
                self.conversation_state.scroll_up(3);
                self.selection_state.clear();
                return None;
            }
            KeyEvent::CtrlChar('j') => {
                self.conversation_state.scroll_down(3);
                self.selection_state.clear();
                return None;
            }
            _ => {}
        }

        // ── Mode 1: Permission pending ──────────────────────────────────────
        if let Some((idx, tool, input, state)) = self.pending_permission.take() {
            match key {
                KeyEvent::Up => {
                    // Put state back with updated selection
                    self.pending_permission = Some((idx, tool, input, {
                        let mut s = state;
                        s.move_up();
                        s
                    }));
                    return None;
                }
                KeyEvent::Down => {
                    self.pending_permission = Some((idx, tool, input, {
                        let mut s = state;
                        s.move_down();
                        s
                    }));
                    return None;
                }
                KeyEvent::Enter => {
                    // Commit the selected option
                    let selected = state.selected_option();
                    let response = match selected {
                        crate::tui::permission::PermissionOption::Deny => PermissionResponse::Deny,
                        crate::tui::permission::PermissionOption::Allow => {
                            PermissionResponse::Allow
                        }
                        crate::tui::permission::PermissionOption::AlwaysAllow => {
                            PermissionResponse::AlwaysAllow
                        }
                    };
                    // Replace the permission block with the result
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
                    let nodes = vec![MarkdownNode::Paragraph {
                        spans: vec![crate::tui::content::InlineSpan::Text(result_text)],
                    }];
                    if idx < self.blocks.len() {
                        self.blocks[idx] = ContentBlock::Markdown { nodes };
                    }
                    let _ = self.event_tx.send(AgentEvent::PermissionResponse(response));
                    return None;
                }
                _ => {
                    // All other keys are swallowed while permission is pending
                    self.pending_permission = Some((idx, tool, input, state));
                    return None;
                }
            }
        }

        // ── Mode 3: Busy -- Ctrl+C interrupts the agent; every other key
        // falls through to the editor so the user can type (and even queue
        // a submission) while the AI is still streaming its response.
        if key == KeyEvent::CtrlC {
            if self.selection_state.has_selection() {
                return None;
            }
            if self.busy {
                let _ = self.event_tx.send(AgentEvent::Interrupt);
                return None;
            }
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
                        self.blocks
                            .push(ContentBlock::UserMessage { text: line.clone() });
                        if let Some(msg_block) = self.blocks.last() {
                            let h = self.block_height(msg_block);
                            self.conversation_state.append_item_height(h);
                            self.conversation_state.auto_scroll();
                        }
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
                return Some(UiAction::Quit);
            }
            EditAction::Interrupt => {
                self.editor.clear();
            }
            EditAction::Continue => {}
        }
        None
    }

    fn render_frame(&mut self) {
        let area = self.renderer.area();
        let is_permission_pending = self.pending_permission.is_some();
        let input_height = if is_permission_pending {
            PermissionWidget::height()
        } else {
            (self.editor.line_count() as u16 + 2).clamp(3, 8)
        };
        let chunks = main_layout(input_height).split(area);
        // chunks: [0]=conversation, [1]=input, [2]=status

        // Conversation area -- update viewport height before rendering
        let conv_area = chunks[0];
        self.conversation_state.viewport_height = conv_area.height;

        // Collect rendering instructions into a temporary list to avoid
        // holding &mut self.renderer at the same time as &mut self.tool_states.
        let visible = self.conversation_state.visible_items();
        let show_spinner = self.busy && self.spinner_start.is_some();
        let spinner_frame = if show_spinner || self.quitting {
            let elapsed = self
                .spinner_start
                .or(self.quitting_start)
                .map(|s| s.elapsed().as_millis() as u64)
                .unwrap_or(0);
            Some(self.spinner.frame_at(elapsed))
        } else {
            None
        };
        let spinner_verb = self.spinner_verb.clone();

        let mut buf = self.renderer.buffer_mut();

        // Conversation area rendering
        // (header removed — cwd/branch shown in status bar at bottom)

        // Render the quitting spinner if in quitting mode
        if self.quitting {
            if let Some(frame) = &spinner_frame {
                let y = conv_area.y + conv_area.height.saturating_sub(1);
                buf.set_str(
                    conv_area.x,
                    y,
                    &format!("{} ", frame),
                    Some(theme::CLAUDE),
                    false,
                );
                buf.set_str(
                    conv_area.x + 2,
                    y,
                    "\u{8fdb}\u{5316}\u{8bb0}\u{5fc6}\u{4e2d}\u{2026} (Ctrl+C \u{5f3a}\u{5236}\u{9000}\u{51fa})",
                    Some(theme::DIM),
                    false,
                );
            }
        } else {
            // Advance welcome animation; mark complete when last row finishes fading in.
            // Last row finishes at: (LOGO_ROWS + INFO_ROWS - 1) * ROW_DELAY + FADE_DURATION
            // = (5 + 5 - 1) * 80 + 200 = 920 ms.
            if let Some(ref anim) = self.welcome_anim {
                let elapsed = anim.start.elapsed().as_millis() as u64;
                let row_delay = WelcomeWidget::ROW_DELAY_MS;
                let fade_duration = WelcomeWidget::FADE_DURATION_MS;
                let last_row_finish_ms = u64::from(anim.total_rows - 1) * row_delay + fade_duration;
                if elapsed > last_row_finish_ms {
                    self.welcome_anim = None;
                }
            }

            let mut tool_visual_idx: usize = 0;

            for vi in &visible {
                if vi.index >= self.blocks.len() {
                    break;
                }
                let block_area = Rect::new(
                    conv_area.x,
                    conv_area.y + vi.viewport_y,
                    conv_area.width.saturating_sub(1), // 1 col for scrollbar
                    vi.visible_rows,
                );
                // Acquire text_map for this block, drop after the call
                let mut text_map = self.renderer.text_map_mut();
                render_block(
                    &self.blocks[vi.index],
                    vi.index,
                    block_area,
                    &mut buf,
                    &mut text_map,
                    &mut tool_visual_idx,
                    &self.focus,
                    &mut self.tool_states,
                    &mut self.welcome_anim,
                );
            }

            // Render spinner at the bottom of the conversation area when busy
            if show_spinner && let Some(frame) = &spinner_frame {
                let y = conv_area.y + conv_area.height.saturating_sub(1);
                buf.set_str(
                    conv_area.x,
                    y,
                    &format!("{} ", frame),
                    Some(theme::CLAUDE),
                    false,
                );
                buf.set_str(
                    conv_area.x + 2,
                    y,
                    &format!("{}\u{2026}", spinner_verb),
                    Some(theme::DIM),
                    false,
                );
            }

            // Scrollbar
            self.conversation_state
                .render_scrollbar(conv_area, &mut buf);
        }

        // Input area: either the permission menu or the normal text editor
        if let Some((_, _, _, ref mut state)) = self.pending_permission {
            // Permission menu: full-height box with centered options
            let perm_widget = PermissionWidget::new("", "");
            perm_widget.render(chunks[1], &mut buf, state);
        } else {
            // Normal input box: top + bottom rounded borders only, dim gray
            let input_block = Block::new()
                .border(BorderStyle::Rounded)
                .borders(BorderSides::HORIZONTAL)
                .border_fg(theme::DIM);
            let input_inner = input_block.inner(chunks[1]);
            input_block.render(chunks[1], &mut buf);

            // Input widget with mode-specific prompt (Claude orange)
            let editor_content = self.editor.content();
            let input_widget = InputWidget::new(
                &editor_content,
                self.editor.cursor_offset(),
                self.editor.mode.prompt(),
            )
            .prompt_fg(theme::CLAUDE);
            input_widget.render(input_inner, &mut buf);
        }

        // Status bar
        let status = StatusWidget {
            cwd: self.cwd.clone(),
            branch: self.branch.clone(),
            model: self.model_name.clone(),
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            spinner_frame: None,
            spinner_verb: String::new(),
        };
        status.render(chunks[2], &mut buf);
    }

    fn block_height(&self, block: &ContentBlock) -> u16 {
        let width = self.renderer.area().width;
        block_height_with_width(block, width)
    }

    fn cleanup(&mut self) -> crate::Result<()> {
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
// Layout helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build the main vertical layout: header (1) + conversation (Fill) + input (dynamic) + status (1).
///
/// `input_height` = min(editor.line_count() + 2, 8), accounting for top+bottom borders.
fn main_layout(input_height: u16) -> Layout {
    Layout::new(Direction::Vertical).constraints(vec![
        Constraint::Fill,
        Constraint::Fixed(input_height),
        Constraint::Fixed(1),
    ])
}

/// Compute a block's height given an explicit width.
fn block_height_with_width(block: &ContentBlock, width: u16) -> u16 {
    match block {
        ContentBlock::UserMessage { text } => {
            use crate::tui::paragraph::{wrap_line, Line, Span};
            let line = Line::from_spans(vec![Span::raw(text.clone())]);
            let effective_width = width.saturating_sub(2) as usize; // "> " prefix
            wrap_line(&line, effective_width.max(1)).len() as u16
        }
        ContentBlock::Markdown { nodes } => MarkdownBlockWidget::height(nodes, width),
        ContentBlock::CodeBlock { code, .. } => CodeBlockWidget::height(code, width),
        ContentBlock::ToolCall { .. } => 1, // folded by default
        ContentBlock::Welcome { .. } => 5,
    }
}

/// Render a single content block into the buffer.
fn render_block(
    block: &ContentBlock,
    block_idx: usize,
    area: Rect,
    buf: &mut Buffer,
    text_map: &mut TextMap,
    tool_idx: &mut usize,
    focus: &FocusManager,
    tool_states: &mut [ToolCallState],
    welcome_anim: &mut Option<WelcomeAnimState>,
) {
    if area.is_empty() {
        return;
    }
    match block {
        ContentBlock::UserMessage { text } => {
            use crate::tui::paragraph::{wrap_line, Line, Span};
            // Render "> " prefix on first row
            buf.set_str(area.x, area.y, "> ", Some(theme::CLAUDE), false);

            // Wrap the user text within available width after prefix
            let effective_width = area.width.saturating_sub(2) as usize;
            let line = Line::from_spans(vec![Span::raw(text.clone())]);
            let rows = wrap_line(&line, effective_width.max(1));

            for (row_idx, row) in rows.iter().enumerate() {
                let y = area.y + row_idx as u16;
                if y >= area.y + area.height {
                    break;
                }
                let start_x = area.x + 2;
                let mut x = start_x;
                let mut global_byte_offset = 0usize;
                for sc in row {
                    if sc.width == 0 {
                        continue;
                    }
                    if x + sc.width > area.x + area.width {
                        break;
                    }
                    let cell = buf.get_mut(x, y);
                    cell.ch = sc.ch;
                    cell.fg = Some(theme::TEXT);
                    cell.bold = false;
                    if sc.width == 2 && x + 1 < area.x + area.width {
                        let cell2 = buf.get_mut(x + 1, y);
                        cell2.ch = '\0';
                        cell2.fg = Some(theme::TEXT);
                    }
                    // Build TextMap: map this screen cell to its text source
                    text_map.set_source(
                        x,
                        y,
                        CellSource {
                            block: block_idx,
                            span: 0,
                            byte_offset: global_byte_offset,
                            width: sc.width,
                        },
                    );
                    global_byte_offset += sc.ch.len_utf8();
                    x += sc.width;
                }
            }
        }
        ContentBlock::Markdown { nodes } => {
            let widget = MarkdownBlockWidget::new(nodes);
            widget.render(area, buf);
        }
        ContentBlock::CodeBlock { language, code } => {
            let widget = CodeBlockWidget::new(code, language.as_deref());
            widget.render(area, buf);
        }
        ContentBlock::ToolCall { name, input, .. } => {
            let current_tool_idx = *tool_idx;
            let summary = extract_input_summary(name, input);
            let focused = focus.is_focused(current_tool_idx);
            let widget = ToolCallWidget::new(name, &summary, input).focused(focused);
            if let Some(state) = tool_states.get_mut(current_tool_idx) {
                widget.render(area, buf, state);
            }
            *tool_idx += 1;
        }
        ContentBlock::Welcome { model, cwd, branch } => {
            let widget = WelcomeWidget::new(model.as_deref(), cwd.as_str(), branch.as_deref());
            // Compute per-row alpha values from the animation state.
            // alpha for info row n = clamp((elapsed - n * ROW_DELAY) / FADE_DURATION, 0.0, 1.0)
            let info_alphas: [f64; WelcomeWidget::INFO_ROWS] = {
                let elapsed_ms = welcome_anim
                    .as_ref()
                    .map(|a| a.start.elapsed().as_millis() as u64)
                    .unwrap_or(u64::MAX);
                let row_delay = WelcomeWidget::ROW_DELAY_MS as f64;
                let fade_duration = WelcomeWidget::FADE_DURATION_MS as f64;
                let mut alphas = [0.0; WelcomeWidget::INFO_ROWS];
                for n in 0..WelcomeWidget::INFO_ROWS {
                    let raw = (elapsed_ms as f64 - (n as f64) * row_delay) / fade_duration;
                    alphas[n] = raw.clamp(0.0, 1.0);
                }
                alphas
            };
            widget.render_with_alpha(area, buf, &info_alphas);
        }
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
