use crate::core::terminal::backend::Backend;
use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::size::TermSize;
use crate::core::terminal::style::theme;
use crate::tui::ansi_serialize::buffer_rows_to_ansi;
use crate::tui::block::{Block, BorderSides, BorderStyle};
use crate::tui::content::MarkdownNode;
use crate::tui::input::{InputMode, InputWidget};
use crate::tui::markdown::MarkdownBlockWidget;
use crate::tui::permission::PermissionState;
use crate::tui::status::{StatusContext, StatusWidget};
use crate::tui::tool_call::{ToolCallState, ToolCallWidget, extract_input_summary};
use crate::tui::widget::{StatefulWidget, Widget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockState {
    Live,
    Committing,
}

pub enum LiveBlock {
    Markdown {
        nodes: Vec<MarkdownNode>,
        state: BlockState,
    },
    ToolCall {
        id: usize,
        name: String,
        input: String,
        output: Option<String>,
        error: Option<String>,
        tc_state: ToolCallState,
        state: BlockState,
    },
    PermissionPrompt {
        tool: String,
        input: String,
        menu: PermissionState,
    },
}

pub struct LiveRegion {
    size: TermSize,
    blocks: Vec<LiveBlock>,
    last_live_rows: u16,
}

impl LiveRegion {
    pub fn new(size: TermSize) -> Self {
        LiveRegion { size, blocks: Vec::new(), last_live_rows: 0 }
    }

    pub fn resize(&mut self, size: TermSize) {
        self.size = size;
    }

    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    pub fn last_live_rows(&self) -> u16 {
        self.last_live_rows
    }

    pub fn push_live_block(&mut self, block: LiveBlock) {
        self.blocks.push(block);
    }

    pub fn mark_last_markdown_committing(&mut self) {
        for b in self.blocks.iter_mut().rev() {
            if let LiveBlock::Markdown { state, .. } = b {
                if *state == BlockState::Live {
                    *state = BlockState::Committing;
                    return;
                }
            }
        }
    }

    /// Remove the trailing block only if it is a `Markdown { state: Live }`.
    /// Used when TextChunk/Done supersedes the previous in-flight view.
    pub fn drop_trailing_live_markdown(&mut self) {
        if let Some(LiveBlock::Markdown { state: BlockState::Live, .. }) = self.blocks.last() {
            self.blocks.pop();
        }
    }

    /// Locate the most recent `ToolCall` whose `tc_state.status` is `Running`
    /// and transition it to Committing with the provided output or error.
    pub fn finish_last_running_tool(
        &mut self,
        output: Option<String>,
        error: Option<String>,
    ) {
        for b in self.blocks.iter_mut().rev() {
            if let LiveBlock::ToolCall {
                state, tc_state, output: o, error: e, ..
            } = b
            {
                if matches!(tc_state.status, crate::tui::tool_call::ToolStatus::Running) {
                    if let Some(err) = &error {
                        let msg = if err.len() > 60 {
                            format!("{}...", &err[..60])
                        } else {
                            err.clone()
                        };
                        *tc_state = crate::tui::tool_call::ToolCallState::new_error(msg);
                        *e = error;
                    } else if let Some(out) = output {
                        let summary = format!("{} chars", out.len());
                        *tc_state = crate::tui::tool_call::ToolCallState::new_success(summary);
                        *o = Some(out);
                    }
                    *state = BlockState::Committing;
                    return;
                }
            }
        }
    }

    /// Remove any `PermissionPrompt` blocks currently in the live region.
    pub fn drop_permission_prompt(&mut self) {
        self.blocks.retain(|b| !matches!(b, LiveBlock::PermissionPrompt { .. }));
    }

    /// Return a mutable reference to the first live `PermissionPrompt`'s menu,
    /// if any.
    pub fn permission_menu_mut(&mut self) -> Option<&mut PermissionState> {
        for b in self.blocks.iter_mut() {
            if let LiveBlock::PermissionPrompt { menu, .. } = b {
                return Some(menu);
            }
        }
        None
    }

    /// Return an immutable reference to the first live `PermissionPrompt`'s
    /// menu, if any.
    pub fn permission_menu(&self) -> Option<&PermissionState> {
        for b in &self.blocks {
            if let LiveBlock::PermissionPrompt { menu, .. } = b {
                return Some(menu);
            }
        }
        None
    }

    pub fn state_at(&self, i: usize) -> Option<BlockState> {
        match self.blocks.get(i)? {
            LiveBlock::Markdown { state, .. } => Some(*state),
            LiveBlock::ToolCall { state, .. } => Some(*state),
            LiveBlock::PermissionPrompt { .. } => Some(BlockState::Live),
        }
    }

    pub fn set_last_live_rows_for_test(&mut self, n: u16) {
        self.last_live_rows = n;
    }

    pub fn commit_text(
        &mut self,
        backend: &mut dyn Backend,
        line: &str,
    ) -> crate::Result<()> {
        self.clear_live_region(backend)?;
        backend.write(line.as_bytes())?;
        // Raw mode has OPOST off, so a bare \n does not move the cursor back
        // to column 0. Write \r\n explicitly to avoid a staircase effect.
        backend.write(b"\r\n")?;
        backend.flush()?;
        Ok(())
    }

    fn clear_live_region(&mut self, backend: &mut dyn Backend) -> crate::Result<()> {
        if self.last_live_rows > 0 {
            let seq = format!("\x1b[{}A\x1b[0J", self.last_live_rows);
            backend.write(seq.as_bytes())?;
            self.last_live_rows = 0;
        }
        Ok(())
    }

    pub fn commit_pending(&mut self, backend: &mut dyn Backend) -> crate::Result<()> {
        let to_commit: Vec<usize> = self
            .blocks
            .iter()
            .enumerate()
            .filter_map(|(i, b)| match b {
                LiveBlock::Markdown { state: BlockState::Committing, .. } => Some(i),
                LiveBlock::ToolCall { state: BlockState::Committing, .. } => Some(i),
                _ => None,
            })
            .collect();
        if to_commit.is_empty() {
            return Ok(());
        }

        self.clear_live_region(backend)?;

        let width = self.size.cols;
        for &i in &to_commit {
            let height = self.block_height(i, width);
            if height == 0 {
                continue;
            }
            let rect = Rect::new(0, 0, width, height);
            let mut buf = Buffer::empty(rect);
            self.render_block_into(i, rect, &mut buf);
            let bytes = buffer_rows_to_ansi(&buf, 0..height);
            backend.write(&bytes)?;
        }
        backend.flush()?;

        for &i in to_commit.iter().rev() {
            self.blocks.remove(i);
        }
        Ok(())
    }

    fn block_height(&self, i: usize, width: u16) -> u16 {
        match &self.blocks[i] {
            LiveBlock::Markdown { nodes, .. } => MarkdownBlockWidget::height(nodes, width),
            LiveBlock::ToolCall { .. } => 1,
            LiveBlock::PermissionPrompt { .. } => crate::tui::permission::PermissionWidget::height(),
        }
    }

    fn render_block_into(&mut self, i: usize, area: Rect, buf: &mut Buffer) {
        match &mut self.blocks[i] {
            LiveBlock::Markdown { nodes, .. } => {
                let w = MarkdownBlockWidget::new(nodes);
                w.render(area, buf);
            }
            LiveBlock::ToolCall { name, input, tc_state, .. } => {
                let summary = extract_input_summary(name, input);
                let w = ToolCallWidget::new(name, &summary, input);
                w.render(area, buf, tc_state);
            }
            LiveBlock::PermissionPrompt { tool, input, menu } => {
                let widget = crate::tui::permission::PermissionWidget::new(tool, input);
                widget.render(area, buf, menu);
            }
        }
    }

    pub fn paint(
        &mut self,
        backend: &mut dyn Backend,
        editor_content: &str,
        cursor_offset: usize,
        mode: InputMode,
        status: &StatusContext,
    ) -> crate::Result<CursorPos> {
        let width = self.size.cols;
        let screen_h = self.size.rows;

        let live_block_rows: u16 = (0..self.blocks.len())
            .map(|i| self.block_height(i, width))
            .sum();

        let editor_lines = editor_content.split('\n').count() as u16;
        let input_h = (editor_lines + 2).clamp(3, 8);
        let blank_row: u16 = if live_block_rows > 0 { 1 } else { 0 };
        let status_h: u16 = 1;

        let live_rows = (live_block_rows + blank_row + input_h + status_h).min(screen_h);
        let top_y = screen_h.saturating_sub(live_rows);

        let area = Rect::new(0, top_y, width, live_rows);
        let mut buf = Buffer::empty(area);

        // Render live blocks starting at top_y.
        let mut y = top_y;
        for i in 0..self.blocks.len() {
            let h = self.block_height(i, width);
            let block_area = Rect::new(0, y, width, h);
            self.render_block_into(i, block_area, &mut buf);
            y = y.saturating_add(h);
        }

        // Blank separator line (only when there are live blocks).
        y = y.saturating_add(blank_row);

        // Input box: Block border + InputWidget inside.
        let input_area = Rect::new(0, y, width, input_h);
        let input_block = Block::new()
            .border(BorderStyle::Rounded)
            .borders(BorderSides::HORIZONTAL)
            .border_fg(theme::DIM);
        let input_inner = input_block.inner(input_area);
        input_block.render(input_area, &mut buf);
        let input_widget = InputWidget::new(editor_content, cursor_offset, mode.prompt())
            .prompt_fg(theme::CLAUDE);
        input_widget.render(input_inner, &mut buf);
        let (cur_x, cur_y) = input_widget.cursor_position(input_inner);
        y = y.saturating_add(input_h);

        // Status bar.
        let status_area = Rect::new(0, y, width, status_h);
        let status_widget = StatusWidget::from_context(status);
        status_widget.render(status_area, &mut buf);

        let mut bytes = buffer_rows_to_ansi(&buf, top_y..top_y + live_rows);
        // paint redraws a bottom-pinned region via absolute cursor positioning.
        // The final \r\n (emitted by buffer_rows_to_ansi for each row) would
        // advance past the last row — at the bottom of the screen that means
        // a scroll. Every keystroke would then push the frame up by one row,
        // producing endless reprinting. Trim it so the last row stays put.
        if bytes.ends_with(b"\r\n") {
            bytes.truncate(bytes.len() - 2);
        }
        backend.write(format!("\x1b[{};1H", top_y + 1).as_bytes())?;
        backend.write(&bytes)?;
        backend.flush()?;

        self.last_live_rows = live_rows;
        Ok(CursorPos { row: cur_y, col: cur_x })
    }

    pub fn frame(
        &mut self,
        backend: &mut dyn Backend,
        editor_content: &str,
        cursor_offset: usize,
        mode: InputMode,
        status: &StatusContext,
    ) -> crate::Result<CursorPos> {
        self.commit_pending(backend)?;
        self.paint(backend, editor_content, cursor_offset, mode, status)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CursorPos {
    pub row: u16,
    pub col: u16,
}
