use crate::core::terminal::backend::Backend;
use crate::core::terminal::size::TermSize;
use crate::tui::content::MarkdownNode;
use crate::tui::permission::PermissionState;
use crate::tui::tool_call::ToolCallState;

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
        backend.write(b"\n")?;
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
}
