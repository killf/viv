use std::sync::mpsc::Receiver;

use crate::bus::{AgentEvent, AgentMessage};
use crate::core::runtime::channel::NotifySender;
use crate::core::terminal::backend::{Backend, CrossBackend};
use crate::core::terminal::buffer::Rect;
use crate::core::terminal::events::{Event, EventLoop};
use crate::core::terminal::input::KeyEvent;
use crate::core::terminal::style::theme;
use crate::tui::block::{Block, BorderSides, BorderStyle};
use crate::tui::code_block::CodeBlockWidget;
use crate::tui::content::{ContentBlock, MarkdownNode, MarkdownParseBuffer};
use crate::tui::conversation::ConversationState;
use crate::tui::focus::{FocusManager, UIMode};
use crate::tui::header::HeaderWidget;
use crate::tui::input::InputWidget;
use crate::tui::layout::{Constraint, Direction, Layout};
use crate::tui::markdown::MarkdownBlockWidget;
use crate::tui::renderer::Renderer;
use crate::tui::spinner::{Spinner, random_verb};
use crate::tui::status::StatusWidget;
use crate::tui::tool_call::{ToolCallState, ToolCallWidget, ToolStatus, extract_input_summary};
use crate::tui::widget::{StatefulWidget, Widget};

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
    editor: LineEditor,
    model_name: String,
    input_tokens: u64,
    output_tokens: u64,
    header: HeaderWidget,
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

    /// (block_idx, tool_name, input_summary) -- stored when PermissionRequest arrives
    pending_permission: Option<(usize, String, String)>,
    /// Set to true after Ctrl+D sends AgentEvent::Quit. While true, we keep the
    /// UI running and wait for AgentMessage::Evolved so the user sees the
    /// "evolving memories" spinner instead of a silent freeze.
    quitting: bool,
    quitting_start: Option<std::time::Instant>,
}

impl TerminalUI {
    pub fn new(
        event_tx: NotifySender<AgentEvent>,
        msg_rx: Receiver<AgentMessage>,
    ) -> crate::Result<Self> {
        let mut backend = CrossBackend::new()?;
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

        let mut blocks = Vec::new();
        let mut conversation_state = ConversationState::new();

        // Push welcome screen as first content block
        blocks.push(ContentBlock::Welcome {
            model: None,
            cwd: header.cwd.clone(),
            branch: header.branch.clone(),
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
            editor: LineEditor::new(),
            model_name: String::new(),
            input_tokens: 0,
            output_tokens: 0,
            header,
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
                // Compute the cursor position before painting, then hand it to
                // the renderer so the final cursor placement is committed in
                // the same synchronized-update block as the diff. This keeps
                // the caret pinned to the input box without toggling cursor
                // visibility, which would otherwise reset the terminal's
                // blink phase on every frame.
                let area = self.renderer.area();
                let input_height = (self.editor.line_count() as u16 + 2).clamp(3, 8);
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
                // Store the permission prompt as a Markdown block
                let prompt_text = format!("  \u{25c6} {}({}) [y/n]", tool, input);
                let nodes = vec![MarkdownNode::Paragraph {
                    spans: vec![crate::tui::content::InlineSpan::Text(prompt_text)],
                }];
                self.blocks.push(ContentBlock::Markdown { nodes });
                let idx = self.blocks.len() - 1;
                self.conversation_state.append_item_height(1);
                self.pending_permission = Some((idx, tool, input));
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
        // ── Mode 1: Permission pending ──────────────────────────────────────
        if let Some((idx, tool, input)) = self.pending_permission.take() {
            let allowed = match key {
                KeyEvent::Char('y') | KeyEvent::Char('Y') => true,
                KeyEvent::Char('n') | KeyEvent::Char('N') => false,
                _ => {
                    // Not a valid response -- put pending_permission back and ignore
                    self.pending_permission = Some((idx, tool, input));
                    return None;
                }
            };
            // Replace the permission block with the result
            let result_text = if allowed {
                format!("  \u{2713} Allowed  {} ({})", tool, input)
            } else {
                format!("  \u{2717} Denied   {} ({})", tool, input)
            };
            let nodes = vec![MarkdownNode::Paragraph {
                spans: vec![crate::tui::content::InlineSpan::Text(result_text)],
            }];
            if idx < self.blocks.len() {
                self.blocks[idx] = ContentBlock::Markdown { nodes };
            }
            let _ = self.event_tx.send(AgentEvent::PermissionResponse(allowed));
            return None;
        }

        // ── Mode 2: Browse mode ─────────────────────────────────────────────
        if self.focus.mode() == UIMode::Browse {
            match key {
                KeyEvent::Escape => {
                    self.focus.exit_browse();
                }
                KeyEvent::Up | KeyEvent::Char('k') => {
                    self.conversation_state.scroll_up(1);
                }
                KeyEvent::Down | KeyEvent::Char('j') => {
                    self.conversation_state.scroll_down(1);
                }
                KeyEvent::Char('g') => {
                    self.conversation_state.scroll_to_top();
                }
                KeyEvent::Char('G') => {
                    self.conversation_state.scroll_to_bottom();
                }
                KeyEvent::Char('n') => {
                    self.focus.next();
                }
                KeyEvent::Enter => {
                    let focus_idx = self.focus.focus_index();
                    if focus_idx < self.tool_states.len() {
                        self.tool_states[focus_idx].toggle_fold();
                        // Recalculate height for the affected block
                        self.recalculate_tool_block_height(focus_idx);
                        self.conversation_state.auto_scroll();
                    }
                }
                _ => {}
            }
            return None;
        }

        // ── Escape -> enter Browse mode (if tool calls exist) ───────────────
        if key == KeyEvent::Escape && !self.busy {
            let tc_count = self.tool_states.len();
            if tc_count > 0 {
                self.focus.enter_browse(tc_count);
                return None;
            }
        }

        // ── Mode 3: Busy -- Ctrl+C interrupts the agent; every other key
        // falls through to the editor so the user can type (and even queue
        // a submission) while the AI is still streaming its response.
        if self.busy && key == KeyEvent::CtrlC {
            let _ = self.event_tx.send(AgentEvent::Interrupt);
            return None;
        }

        // ── Mode 4: Normal editing (busy or idle) ───────────────────────────
        let action = self.editor.handle_key(key);
        match action {
            EditAction::Submit(line) => {
                if !line.trim().is_empty() {
                    self.blocks
                        .push(ContentBlock::UserMessage { text: line.clone() });
                    if let Some(msg_block) = self.blocks.last() {
                        let h = self.block_height(msg_block);
                        self.conversation_state.append_item_height(h);
                        self.conversation_state.auto_scroll();
                    }
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
        let input_height = (self.editor.line_count() as u16 + 2).clamp(3, 8);
        let chunks = main_layout(input_height).split(area);
        // chunks: [0]=header, [1]=conversation, [2]=input, [3]=status

        // Conversation area -- update viewport height before rendering
        let conv_area = chunks[1];
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

        let buf = self.renderer.buffer_mut();

        // Header bar
        self.header.render(chunks[0], buf);

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
                render_block(
                    &self.blocks[vi.index],
                    block_area,
                    buf,
                    &mut tool_visual_idx,
                    &self.focus,
                    &mut self.tool_states,
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
            self.conversation_state.render_scrollbar(conv_area, buf);
        }

        // Input box: top + bottom rounded borders only, dim gray
        let input_block = Block::new()
            .border(BorderStyle::Rounded)
            .borders(BorderSides::HORIZONTAL)
            .border_fg(theme::DIM);
        let input_inner = input_block.inner(chunks[2]);
        input_block.render(chunks[2], buf);

        // Input widget with > prompt (Claude orange)
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

    fn block_height(&self, block: &ContentBlock) -> u16 {
        let width = self.renderer.area().width;
        block_height_with_width(block, width)
    }

    /// Recalculate the height of the block corresponding to tool call at `tool_idx`.
    fn recalculate_tool_block_height(&mut self, tool_idx: usize) {
        // Find the block index for the Nth ToolCall
        let mut tc_count = 0;
        for (block_idx, block) in self.blocks.iter().enumerate() {
            if let ContentBlock::ToolCall { input, .. } = block {
                if tc_count == tool_idx {
                    let h = if let Some(state) = self.tool_states.get(tool_idx) {
                        if state.folded {
                            1
                        } else {
                            // header row + input block (lines + 2 borders)
                            let content_lines = input.split('\n').count() as u16;
                            1 + content_lines + 2
                        }
                    } else {
                        1
                    };
                    self.conversation_state.set_item_height(block_idx, h);
                    return;
                }
                tc_count += 1;
            }
        }
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
        Constraint::Fixed(1),
        Constraint::Fill,
        Constraint::Fixed(input_height),
        Constraint::Fixed(1),
    ])
}

/// Compute a block's height given an explicit width.
fn block_height_with_width(block: &ContentBlock, width: u16) -> u16 {
    match block {
        ContentBlock::UserMessage { text } => {
            use crate::tui::paragraph::{Line, Span, wrap_line};
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

/// Render a single content block into the buffer. Free function to avoid
/// borrowing the entire TerminalUI while we hold &mut Buffer.
fn render_block(
    block: &ContentBlock,
    area: Rect,
    buf: &mut crate::core::terminal::buffer::Buffer,
    tool_idx: &mut usize,
    focus: &FocusManager,
    tool_states: &mut [ToolCallState],
) {
    if area.is_empty() {
        return;
    }
    match block {
        ContentBlock::UserMessage { text } => {
            use crate::tui::paragraph::{Line, Span, wrap_line};
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
            use crate::tui::welcome::WelcomeWidget;
            let widget = WelcomeWidget::new(model.as_deref(), cwd.as_str(), branch.as_deref());
            widget.render(area, buf);
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
}

impl LineEditor {
    pub fn new() -> Self {
        LineEditor {
            lines: vec![String::new()],
            row: 0,
            col: 0,
        }
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
