# Inline 渲染（半全屏）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate `TerminalUI` from alt-screen full-screen TUI to Claude Code–style inline rendering: completed content flows into the terminal's native scrollback; a bottom-pinned `LiveRegion` redraws every frame.

**Architecture:** Introduce a `LiveRegion` primitive under `src/tui/live_region.rs` that owns `live_blocks + editor + status` and exposes `frame(backend, editor, ...)`. Each frame: `cursor_up(last_live_rows) + clear_to_end` → write committing blocks as ANSI bytes (they scroll up into scrollback) → repaint live area → position cursor inside input box. `TerminalUI` delegates all rendering to `LiveRegion` and no longer touches alt-screen, mouse, or scroll state.

**Tech Stack:** Rust 2024 edition, no external crates. Reuses existing `Backend` trait, `Buffer`, `Renderer`, widget modules (`MarkdownBlockWidget`, `CodeBlockWidget`, `ToolCallWidget`, `InputWidget`, `StatusWidget`, `PermissionWidget`, `WelcomeWidget`), and `MarkdownParseBuffer`.

**Spec:** `docs/superpowers/specs/2026-04-22-inline-rendering-design.md`

**Repository conventions (from CLAUDE.md + memory):**
- No `unwrap`/`expect`/`panic!`/`unreachable!` in `src/`; all errors flow through `crate::Result`
- Tests live in `tests/`, mirroring `src/`; no `#[cfg(test)]`
- `cargo test` must pass after every task before commit
- No external crates
- Each `#[test]` ≤ 10 s

---

## File Structure

**Created:**
- `src/tui/live_region.rs` — `LiveBlock`, `BlockState`, `LiveRegion` core type
- `src/tui/ansi_serialize.rs` — `buffer_to_ansi_bytes(buf, row_range)` helper
- `tests/tui/live_region_test.rs` — unit tests for `LiveRegion`
- `tests/tui/ansi_serialize_test.rs` — unit tests for the serializer
- `tests/tui/inline_flow_test.rs` — scripted-conversation integration test

**Modified:**
- `src/tui/mod.rs` — add `live_region`, `ansi_serialize`; remove `focus`, `selection`, `text_map`, `conversation`
- `src/tui/terminal.rs` — remove alt-screen calls, mouse handling, scroll keys; route all rendering through `LiveRegion`
- `src/tui/status.rs` — extend `StatusWidget` with optional `spinner: Option<(char, String)>`
- `src/tui/renderer.rs` — drop `text_map` field
- `src/core/terminal/events.rs` — stop enabling mouse tracking
- `tests/tui/mod.rs` — drop removed-module tests
- `tests/tui/terminal_test.rs` — update for new rendering model
- `tests/tui/renderer_test.rs` — drop text_map assertions

**Deleted:**
- `src/tui/focus.rs`, `src/tui/selection.rs`, `src/tui/text_map.rs`, `src/tui/conversation.rs`
- `tests/tui/focus_test.rs`, `tests/tui/selection_test.rs`, `tests/tui/text_map_test.rs`, `tests/tui/conversation_test.rs`

---

## Task 1: Scaffold `LiveRegion` types

**Files:**
- Create: `src/tui/live_region.rs`
- Create: `tests/tui/live_region_test.rs`
- Modify: `src/tui/mod.rs`
- Modify: `tests/tui/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/tui/live_region_test.rs`:

```rust
use viv::tui::live_region::{BlockState, LiveBlock, LiveRegion};
use viv::tui::content::{InlineSpan, MarkdownNode};
use viv::core::terminal::size::TermSize;

#[test]
fn new_region_has_no_blocks_and_zero_last_live_rows() {
    let region = LiveRegion::new(TermSize { cols: 80, rows: 24 });
    assert_eq!(region.block_count(), 0);
    assert_eq!(region.last_live_rows(), 0);
}

#[test]
fn push_live_block_appends_with_live_state() {
    let mut region = LiveRegion::new(TermSize { cols: 80, rows: 24 });
    let nodes = vec![MarkdownNode::Paragraph {
        spans: vec![InlineSpan::Text("hello".into())],
    }];
    region.push_live_block(LiveBlock::Markdown { nodes, state: BlockState::Live });
    assert_eq!(region.block_count(), 1);
}

#[test]
fn mark_last_markdown_committing_transitions_state() {
    let mut region = LiveRegion::new(TermSize { cols: 80, rows: 24 });
    let nodes = vec![MarkdownNode::Paragraph {
        spans: vec![InlineSpan::Text("hi".into())],
    }];
    region.push_live_block(LiveBlock::Markdown { nodes, state: BlockState::Live });
    region.mark_last_markdown_committing();
    assert_eq!(region.state_at(0), Some(BlockState::Committing));
}
```

Register the test module by adding this line to `tests/tui/mod.rs` (alphabetical order):
```rust
mod live_region_test;
```

- [ ] **Step 2: Create the module file**

Create `src/tui/live_region.rs`:

```rust
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
}
```

Register the module by adding this line to `src/tui/mod.rs` (alphabetical order):
```rust
pub mod live_region;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --test tui live_region_test`
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/tui/live_region.rs src/tui/mod.rs tests/tui/live_region_test.rs tests/tui/mod.rs
git commit -m "feat(tui): scaffold LiveRegion types for inline rendering"
```

---

## Task 2: ANSI byte serializer for `Buffer`

**Files:**
- Create: `src/tui/ansi_serialize.rs`
- Create: `tests/tui/ansi_serialize_test.rs`
- Modify: `src/tui/mod.rs`, `tests/tui/mod.rs`

Goal: convert a contiguous row range of a `Buffer` into a byte stream — each row emits style-change escapes (SGR), character bytes, `\x1b[0m\n` at row end. Used to commit blocks into scrollback.

- [ ] **Step 1: Write the failing test**

Create `tests/tui/ansi_serialize_test.rs`:

```rust
use viv::core::terminal::buffer::{Buffer, Rect};
use viv::tui::ansi_serialize::buffer_rows_to_ansi;

#[test]
fn serializes_plain_ascii_row_then_newline() {
    let mut buf = Buffer::new(Rect::new(0, 0, 5, 1));
    buf.set_str(0, 0, "hello", None, false);
    let out = buffer_rows_to_ansi(&buf, 0..1);
    // Trailing reset + newline, ASCII body intact.
    assert!(out.ends_with(b"\x1b[0m\n"));
    assert!(out.windows(5).any(|w| w == b"hello"));
}

#[test]
fn collapses_trailing_blanks() {
    let mut buf = Buffer::new(Rect::new(0, 0, 10, 1));
    buf.set_str(0, 0, "hi", None, false);
    let out = buffer_rows_to_ansi(&buf, 0..1);
    // Should NOT pad to full width with spaces before the newline.
    let body = std::str::from_utf8(&out).expect("utf8");
    assert!(!body.contains("          \x1b[0m\n"),
        "expected trailing blanks trimmed, got {:?}", body);
}

#[test]
fn emits_one_line_per_row_in_range() {
    let mut buf = Buffer::new(Rect::new(0, 0, 4, 3));
    buf.set_str(0, 0, "aaa", None, false);
    buf.set_str(0, 1, "bbb", None, false);
    buf.set_str(0, 2, "ccc", None, false);
    let out = buffer_rows_to_ansi(&buf, 0..3);
    let nl_count = out.iter().filter(|&&b| b == b'\n').count();
    assert_eq!(nl_count, 3);
}
```

Add to `tests/tui/mod.rs`: `mod ansi_serialize_test;`

- [ ] **Step 2: Inspect the `Buffer` API for reference**

Run: `grep -n "pub fn\|pub struct" src/core/terminal/buffer.rs | head -30`
Use `Buffer::get(x, y)` returning a `Cell` with `ch: char`, `fg: Option<Color>`, `bg: Option<Color>`, `bold: bool`.

- [ ] **Step 3: Implement**

Create `src/tui/ansi_serialize.rs`:

```rust
use std::ops::Range;

use crate::core::terminal::buffer::Buffer;
use crate::core::terminal::style::Color;

/// Serialize a row range of `buf` as ANSI bytes. Each row is terminated by
/// `ESC[0m\n`. Trailing blank cells are trimmed so committed lines don't
/// pad to full width (which would make scrollback look awkward).
pub fn buffer_rows_to_ansi(buf: &Buffer, rows: Range<u16>) -> Vec<u8> {
    let mut out = Vec::with_capacity((rows.end - rows.start) as usize * buf.width() as usize);
    for y in rows {
        let mut last_fg: Option<Color> = None;
        let mut last_bold = false;
        let mut end_x = 0u16;
        for x in 0..buf.width() {
            let cell = buf.get(x, y);
            if cell.ch != ' ' && cell.ch != '\0' {
                end_x = x + 1;
            }
        }
        // Emit cells [0, end_x), skipping continuation cells ('\0').
        for x in 0..end_x {
            let cell = buf.get(x, y);
            if cell.ch == '\0' {
                continue;
            }
            if cell.fg != last_fg || cell.bold != last_bold {
                out.extend_from_slice(b"\x1b[0m");
                if cell.bold {
                    out.extend_from_slice(b"\x1b[1m");
                }
                if let Some(c) = cell.fg {
                    out.extend_from_slice(sgr_fg(c).as_bytes());
                }
                last_fg = cell.fg;
                last_bold = cell.bold;
            }
            let mut buf4 = [0u8; 4];
            let s = cell.ch.encode_utf8(&mut buf4);
            out.extend_from_slice(s.as_bytes());
        }
        out.extend_from_slice(b"\x1b[0m\n");
    }
    out
}

fn sgr_fg(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("\x1b[38;2;{};{};{}m", r, g, b),
        Color::Indexed(i) => format!("\x1b[38;5;{}m", i),
        Color::Reset => "\x1b[39m".to_string(),
    }
}
```

If `Color` has different variants in `src/core/terminal/style.rs`, adapt `sgr_fg` to match them — grep `Color::` in the module to see the set.

Add to `src/tui/mod.rs`: `pub mod ansi_serialize;`

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test --test tui ansi_serialize_test`
Expected: 3 tests pass.

- [ ] **Step 5: Run full suite**

Run: `cargo test`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/tui/ansi_serialize.rs src/tui/mod.rs tests/tui/ansi_serialize_test.rs tests/tui/mod.rs
git commit -m "feat(tui): add buffer-to-ANSI serializer for scrollback commits"
```

---

## Task 3: `LiveRegion::commit_text` — direct scrollback append

**Files:**
- Modify: `src/tui/live_region.rs`
- Modify: `tests/tui/live_region_test.rs`

Goal: append a plain-text committed line (for `UserMessage`, `Status`, `Error`) directly to backend, preceded by a live-region clear.

- [ ] **Step 1: Write the failing test**

Append to `tests/tui/live_region_test.rs`:

```rust
use viv::core::terminal::backend::{Backend, TestBackend};

#[test]
fn commit_text_clears_live_region_then_writes_line() {
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    let mut backend = TestBackend::new(40, 10);
    // Simulate a prior frame that painted 3 rows of live content.
    region.set_last_live_rows_for_test(3);
    region.commit_text(&mut backend, "> hello world").unwrap();

    let out = String::from_utf8(backend.output.clone()).unwrap();
    // Expect: cursor_up(3) + clear_to_end + line + \n.
    assert!(out.starts_with("\x1b[3A\x1b[0J"));
    assert!(out.contains("> hello world"));
    assert!(out.ends_with("\n"));
    // last_live_rows should be reset to 0 since live region was cleared.
    assert_eq!(region.last_live_rows(), 0);
}

#[test]
fn commit_text_with_zero_live_rows_skips_cursor_up() {
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    let mut backend = TestBackend::new(40, 10);
    region.commit_text(&mut backend, "hi").unwrap();
    let out = String::from_utf8(backend.output.clone()).unwrap();
    // No cursor-up sequence when nothing was painted.
    assert!(!out.contains("\x1b[0A"));
    assert!(out.contains("hi\n"));
}
```

- [ ] **Step 2: Implement**

Add to `src/tui/live_region.rs`:

```rust
use crate::core::terminal::backend::Backend;

impl LiveRegion {
    /// Test-only: seed `last_live_rows` to simulate a prior frame.
    pub fn set_last_live_rows_for_test(&mut self, n: u16) {
        self.last_live_rows = n;
    }

    /// Clear the live region (if any) from the screen, then write `line\n`
    /// into scrollback. Resets `last_live_rows` to 0; the caller is expected
    /// to call `frame()` next to repaint the live region.
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
```

- [ ] **Step 3: Run tests**

Run: `cargo test --test tui live_region_test`
Expected: 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/tui/live_region.rs tests/tui/live_region_test.rs
git commit -m "feat(tui): LiveRegion::commit_text appends plain lines to scrollback"
```

---

## Task 4: `LiveRegion::commit_pending` — render `Committing` blocks to scrollback

**Files:**
- Modify: `src/tui/live_region.rs`
- Modify: `tests/tui/live_region_test.rs`

Goal: render any `Committing` blocks into a temporary `Buffer`, serialize through `buffer_rows_to_ansi`, write to backend, remove from `blocks`.

- [ ] **Step 1: Write the failing test**

Append to `tests/tui/live_region_test.rs`:

```rust
#[test]
fn commit_pending_writes_markdown_then_removes_block() {
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    let nodes = vec![MarkdownNode::Paragraph {
        spans: vec![InlineSpan::Text("hello".into())],
    }];
    region.push_live_block(LiveBlock::Markdown { nodes, state: BlockState::Committing });
    let mut backend = TestBackend::new(40, 10);
    region.commit_pending(&mut backend).unwrap();

    assert_eq!(region.block_count(), 0);
    let out = String::from_utf8(backend.output.clone()).unwrap();
    assert!(out.contains("hello"), "got {:?}", out);
    assert!(out.ends_with("\n"));
}

#[test]
fn commit_pending_leaves_live_blocks_untouched() {
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    let nodes = vec![MarkdownNode::Paragraph {
        spans: vec![InlineSpan::Text("staying".into())],
    }];
    region.push_live_block(LiveBlock::Markdown { nodes, state: BlockState::Live });
    let mut backend = TestBackend::new(40, 10);
    region.commit_pending(&mut backend).unwrap();
    assert_eq!(region.block_count(), 1);
}
```

- [ ] **Step 2: Implement**

Add to `src/tui/live_region.rs`:

```rust
use crate::core::terminal::buffer::{Buffer, Rect};
use crate::tui::ansi_serialize::buffer_rows_to_ansi;
use crate::tui::code_block::CodeBlockWidget;
use crate::tui::markdown::MarkdownBlockWidget;
use crate::tui::tool_call::{extract_input_summary, ToolCallWidget};
use crate::tui::widget::{StatefulWidget, Widget};

impl LiveRegion {
    /// Commit every block currently in `Committing` state: render into a
    /// scratch buffer, serialize, write to backend. Removes them from
    /// `self.blocks` afterwards.
    pub fn commit_pending(&mut self, backend: &mut dyn Backend) -> crate::Result<()> {
        // Snapshot indices of blocks to commit (in insertion order).
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
            let rect = Rect::new(0, 0, width, height);
            let mut buf = Buffer::new(rect);
            self.render_block_into(i, rect, &mut buf);
            let bytes = buffer_rows_to_ansi(&buf, 0..height);
            backend.write(&bytes)?;
        }
        backend.flush()?;

        // Remove in reverse so indices stay valid.
        for &i in to_commit.iter().rev() {
            self.blocks.remove(i);
        }
        Ok(())
    }

    fn block_height(&self, i: usize, width: u16) -> u16 {
        match &self.blocks[i] {
            LiveBlock::Markdown { nodes, .. } => MarkdownBlockWidget::height(nodes, width),
            LiveBlock::ToolCall { .. } => 1,
            LiveBlock::PermissionPrompt { .. } => 1,
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
            LiveBlock::PermissionPrompt { tool, input, .. } => {
                // A committed permission block is replaced with a result line
                // before commit (see Task 9); this branch is defensive.
                let text = format!("  \u{25c6} {}({})", tool, input);
                buf.set_str(0, 0, &text, None, false);
            }
        }
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --test tui live_region_test`
Expected: 7 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/tui/live_region.rs tests/tui/live_region_test.rs
git commit -m "feat(tui): LiveRegion::commit_pending writes blocks into scrollback"
```

---

## Task 5: `LiveRegion::paint` — render live area to backend

**Files:**
- Modify: `src/tui/live_region.rs`
- Modify: `tests/tui/live_region_test.rs`

Goal: paint `[live_blocks] + [blank line] + [input box] + [status]` at the bottom of the screen. Returns the `(row, col)` cursor position inside the input box.

- [ ] **Step 1: Write the failing test**

Append to `tests/tui/live_region_test.rs`:

```rust
use viv::tui::input::InputMode;
use viv::tui::status::StatusContext;

#[test]
fn paint_returns_cursor_inside_input_and_updates_last_live_rows() {
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    let ctx = StatusContext {
        cwd: "~/p".into(),
        branch: None,
        model: "m".into(),
        input_tokens: 0,
        output_tokens: 0,
        spinner_frame: None,
        spinner_verb: String::new(),
    };
    let mut backend = TestBackend::new(40, 10);
    let cur = region.paint(&mut backend, "", 0, InputMode::Chat, &ctx).unwrap();
    // Live region occupies at least input(3) + status(1) = 4 rows.
    assert!(region.last_live_rows() >= 4);
    // Cursor y should land inside the input box (row 0-indexed within screen).
    assert!(cur.row < 10);
}

#[test]
fn paint_includes_in_flight_markdown_block() {
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    let nodes = vec![MarkdownNode::Paragraph {
        spans: vec![InlineSpan::Text("streaming…".into())],
    }];
    region.push_live_block(LiveBlock::Markdown { nodes, state: BlockState::Live });
    let ctx = StatusContext {
        cwd: "~/p".into(),
        branch: None,
        model: "m".into(),
        input_tokens: 0,
        output_tokens: 0,
        spinner_frame: None,
        spinner_verb: String::new(),
    };
    let mut backend = TestBackend::new(40, 10);
    region.paint(&mut backend, "", 0, InputMode::Chat, &ctx).unwrap();
    // Live rows now include the markdown block (height ≥ 1).
    assert!(region.last_live_rows() >= 5);
}
```

- [ ] **Step 2: Extend `StatusWidget` to accept `StatusContext`**

Modify `src/tui/status.rs`. Add near the top:

```rust
pub struct StatusContext {
    pub cwd: String,
    pub branch: Option<String>,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub spinner_frame: Option<char>,
    pub spinner_verb: String,
}
```

Then teach `StatusWidget::render` to draw the spinner prefix (if `spinner_frame.is_some()`) in Claude-orange followed by `verb…`, then a `·` separator, then the original cwd/branch/model/tokens content. Preserve existing fields for back-compat or replace the struct entirely — check `tests/tui/status_test.rs` for any fields you rename and update accordingly.

- [ ] **Step 3: Implement `LiveRegion::paint`**

Add to `src/tui/live_region.rs`:

```rust
use crate::tui::block::{Block, BorderSides, BorderStyle};
use crate::tui::input::{InputMode, InputWidget};
use crate::tui::status::{StatusContext, StatusWidget};
use crate::core::terminal::style::theme;

#[derive(Debug, Clone, Copy)]
pub struct CursorPos {
    pub row: u16,
    pub col: u16,
}

impl LiveRegion {
    /// Paint the live region at the bottom of the screen. Returns the cursor
    /// position (screen-absolute) inside the input box.
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

        // Compute live block rows.
        let live_block_rows: u16 = (0..self.blocks.len())
            .map(|i| self.block_height(i, width))
            .sum();

        // Input height: lines_of(editor_content) + 2 (borders), clamped to [3, 8].
        let editor_lines = editor_content.split('\n').count() as u16;
        let input_h = (editor_lines + 2).clamp(3, 8);
        let blank_row: u16 = if live_block_rows > 0 { 1 } else { 0 };
        let status_h: u16 = 1;

        let live_rows = live_block_rows + blank_row + input_h + status_h;
        let live_rows = live_rows.min(screen_h);

        let top_y = screen_h.saturating_sub(live_rows);

        // Paint into a Buffer sized to the live region.
        let area = Rect::new(0, top_y, width, live_rows);
        let mut buf = Buffer::new(area);

        // 1. Live blocks
        let mut y = top_y;
        for i in 0..self.blocks.len() {
            let h = self.block_height(i, width);
            let block_area = Rect::new(0, y, width, h);
            self.render_block_into(i, block_area, &mut buf);
            y += h;
        }
        // 2. Blank line (skipped if no live blocks)
        y += blank_row;
        // 3. Input box
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
        let cursor = input_widget.cursor_position(input_inner);
        y += input_h;
        // 4. Status
        let status_area = Rect::new(0, y, width, status_h);
        let status_widget = StatusWidget::from_context(status);
        status_widget.render(status_area, &mut buf);

        // Serialize and write.
        let bytes = buffer_rows_to_ansi(&buf, 0..live_rows);
        backend.write(format!("\x1b[{};1H", top_y + 1).as_bytes())?;
        backend.write(&bytes)?;
        backend.flush()?;

        self.last_live_rows = live_rows;
        Ok(CursorPos { row: cursor.y, col: cursor.x })
    }
}
```

`StatusWidget::from_context(ctx)` is a new constructor — add it when you extend `StatusWidget` in Step 2.

- [ ] **Step 4: Run tests**

Run: `cargo test --test tui live_region_test`
Expected: 9 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/tui/live_region.rs src/tui/status.rs tests/tui/live_region_test.rs tests/tui/status_test.rs
git commit -m "feat(tui): LiveRegion::paint draws bottom-pinned live area"
```

---

## Task 6: `LiveRegion::frame` — full-frame entry point

**Files:**
- Modify: `src/tui/live_region.rs`
- Modify: `tests/tui/live_region_test.rs`

- [ ] **Step 1: Write the failing test**

Append to `tests/tui/live_region_test.rs`:

```rust
#[test]
fn frame_commits_then_paints_and_returns_cursor() {
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    // Queue one committing markdown + one live tool call.
    let nodes = vec![MarkdownNode::Paragraph {
        spans: vec![InlineSpan::Text("done".into())],
    }];
    region.push_live_block(LiveBlock::Markdown { nodes, state: BlockState::Committing });
    let ctx = StatusContext {
        cwd: "~/p".into(), branch: None, model: "m".into(),
        input_tokens: 0, output_tokens: 0,
        spinner_frame: None, spinner_verb: String::new(),
    };
    let mut backend = TestBackend::new(40, 10);
    let cur = region.frame(&mut backend, "", 0, InputMode::Chat, &ctx).unwrap();

    // Committed block is gone; zero blocks left.
    assert_eq!(region.block_count(), 0);
    let out = String::from_utf8(backend.output.clone()).unwrap();
    assert!(out.contains("done"));
    assert!(cur.row < 10);
    assert!(region.last_live_rows() > 0);
}
```

- [ ] **Step 2: Implement**

Add to `src/tui/live_region.rs`:

```rust
impl LiveRegion {
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
```

- [ ] **Step 3: Run tests**

Run: `cargo test --test tui live_region_test`
Expected: 10 tests pass.

- [ ] **Step 4: Full suite green?**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/tui/live_region.rs tests/tui/live_region_test.rs
git commit -m "feat(tui): LiveRegion::frame = commit + paint"
```

---

## Task 7: `TerminalUI::new` stops calling `enter_alt_screen`; hold `LiveRegion`

**Files:**
- Modify: `src/tui/terminal.rs`

At this point the new `LiveRegion` exists but is unused. This task is the first behavior change: stop entering alt-screen. Conversation rendering still uses old code paths; we'll cut those over in the next tasks.

- [ ] **Step 1: Locate the call**

Read `src/tui/terminal.rs:123-140`.

- [ ] **Step 2: Remove the alt-screen enter and add `LiveRegion` field**

In `TerminalUI`'s struct, add:
```rust
live_region: crate::tui::live_region::LiveRegion,
```

In `TerminalUI::new`, delete line 128 (`backend.enter_alt_screen()?;`). Keep `enable_raw_mode`. Initialize the field:
```rust
live_region: crate::tui::live_region::LiveRegion::new(size),
```

In `TerminalUI::cleanup`, delete `self.backend.leave_alt_screen()?;` (line 808).

- [ ] **Step 3: Run full suite**

Run: `cargo test`
Expected: all pass (no observable behavior change in tests because `TestBackend::in_alt_screen` tracking is no longer touched but isn't asserted by any test after this edit).

If `tests/tui/terminal_test.rs` asserts `backend.in_alt_screen == true`, change those assertions to `== false`.

- [ ] **Step 4: Manual smoke test**

Run: `cargo run` (requires `VIV_API_KEY`; if unavailable, skip to Step 5 and rely on the integration test in Task 12).
Expected: viv launches without clearing your terminal; on Ctrl+D exit, your prior shell output is still visible and the conversation is in scrollback.

- [ ] **Step 5: Commit**

```bash
git add src/tui/terminal.rs tests/tui/terminal_test.rs
git commit -m "refactor(tui): stop entering alt-screen; hold a LiveRegion"
```

---

## Task 8: Route `TextChunk` / `Done` through `LiveRegion`

**Files:**
- Modify: `src/tui/terminal.rs`

- [ ] **Step 1: Replace `AgentMessage::TextChunk` handler**

In `handle_agent_message`, locate the `AgentMessage::TextChunk(s)` arm (around line 365). Replace its body with:

```rust
let new_nodes = self.parse_buffer.push(&s);
// Every node produced by the parse buffer is a CLOSED markdown unit — commit it.
for nodes in new_nodes.into_iter().map(nodes_of_block) {
    if let Some(nodes) = nodes {
        self.live_region.push_live_block(
            crate::tui::live_region::LiveBlock::Markdown {
                nodes,
                state: crate::tui::live_region::BlockState::Committing,
            },
        );
    }
}
// Any un-closed in-flight prefix the parse buffer is still holding: render
// it as a Live block so the user sees it streaming. We do this by flushing
// a "peek" copy each frame — add `MarkdownParseBuffer::peek_pending() -> Vec<MarkdownNode>`
// in a minor edit if not present (check `src/tui/content.rs`).
let pending = self.parse_buffer.peek_pending();
// Remove any previous in-flight Live markdown (last block if Live+Markdown).
self.live_region.drop_trailing_live_markdown();
if !pending.is_empty() {
    self.live_region.push_live_block(
        crate::tui::live_region::LiveBlock::Markdown {
            nodes: pending,
            state: crate::tui::live_region::BlockState::Live,
        },
    );
}
```

`nodes_of_block` is a local helper:
```rust
fn nodes_of_block(block: crate::tui::content::ContentBlock) -> Option<Vec<crate::tui::content::MarkdownNode>> {
    match block {
        crate::tui::content::ContentBlock::Markdown { nodes } => Some(nodes),
        _ => None,
    }
}
```

Add `drop_trailing_live_markdown` to `LiveRegion`:
```rust
pub fn drop_trailing_live_markdown(&mut self) {
    if let Some(LiveBlock::Markdown { state: BlockState::Live, .. }) = self.blocks.last() {
        self.blocks.pop();
    }
}
```

If `MarkdownParseBuffer::peek_pending` does not exist, add it in `src/tui/content.rs`. Its job: render the currently-buffered bytes as a best-effort `Vec<MarkdownNode>` without advancing the buffer state.

- [ ] **Step 2: Replace `AgentMessage::Done` handler**

Replace its body with:
```rust
let remaining = self.parse_buffer.flush();
for nodes in remaining.into_iter().filter_map(nodes_of_block) {
    self.live_region.push_live_block(
        crate::tui::live_region::LiveBlock::Markdown {
            nodes,
            state: crate::tui::live_region::BlockState::Committing,
        },
    );
}
self.live_region.drop_trailing_live_markdown();
self.busy = false;
self.spinner_start = None;
```

- [ ] **Step 3: Write the test**

Append to `tests/tui/live_region_test.rs`:
```rust
#[test]
fn drop_trailing_live_markdown_removes_only_trailing_live() {
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    let nodes = vec![MarkdownNode::Paragraph {
        spans: vec![InlineSpan::Text("a".into())],
    }];
    region.push_live_block(LiveBlock::Markdown {
        nodes: nodes.clone(), state: BlockState::Committing,
    });
    region.push_live_block(LiveBlock::Markdown {
        nodes, state: BlockState::Live,
    });
    region.drop_trailing_live_markdown();
    assert_eq!(region.block_count(), 1);
    assert_eq!(region.state_at(0), Some(BlockState::Committing));
}
```

- [ ] **Step 4: Run full suite**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/tui/terminal.rs src/tui/live_region.rs src/tui/content.rs tests/tui/live_region_test.rs
git commit -m "feat(tui): route TextChunk/Done through LiveRegion"
```

---

## Task 9: Route `UserMessage`/`Status`/`Error`/`ToolStart`/`ToolEnd`/`ToolError`/`PermissionRequest`/`PermissionResponse` through `LiveRegion`

**Files:**
- Modify: `src/tui/terminal.rs`

- [ ] **Step 1: Replace user message commit**

In the `EditAction::Submit(line)` branch (around line 610), replace the `self.blocks.push(ContentBlock::UserMessage { text: line.clone() })` block with:

```rust
if !is_command {
    let text = format!("> {}", line);
    self.live_region.commit_text(&mut self.backend, &text)?;
    self.editor.push_history(line.clone());
}
```

(`handle_key` now needs `crate::Result<Option<UiAction>>`; propagate the `?`. Update the call site in `run()` to handle the `Result`.)

- [ ] **Step 2: Replace `Status` and `Error` handlers**

In `handle_agent_message`, replace the `Status(s)` body with:
```rust
self.live_region.commit_text(&mut self.backend, &s)?;
```
and the `Error(e)` body with:
```rust
let msg = format!("\u{25cf} error: {}", e);
self.live_region.commit_text(&mut self.backend, &msg)?;
self.busy = false;
self.spinner_start = None;
```

`handle_agent_message` now needs to return `crate::Result<()>`; propagate `?` and update the call site.

- [ ] **Step 3: Replace tool-call handlers**

`ToolStart { name, input }`:
```rust
let id = self.tool_seq;
self.tool_seq += 1;
self.live_region.push_live_block(
    crate::tui::live_region::LiveBlock::ToolCall {
        id, name, input,
        output: None, error: None,
        tc_state: crate::tui::tool_call::ToolCallState::new_running(),
        state: crate::tui::live_region::BlockState::Live,
    },
);
```

`ToolEnd { output, .. }`: find the most recent `LiveBlock::ToolCall` with `tc_state.status == Running`, update `output`, transition `tc_state` to success, set `state = Committing`.

`ToolError { error, .. }`: same, but set error + failure state + `state = Committing`.

Add helpers to `LiveRegion`:
```rust
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
```

Use it in `handle_agent_message`:
```rust
AgentMessage::ToolEnd { name: _, output } => {
    self.live_region.finish_last_running_tool(Some(output), None);
}
AgentMessage::ToolError { name: _, error } => {
    self.live_region.finish_last_running_tool(None, Some(error));
}
```

- [ ] **Step 4: Replace permission handlers**

`PermissionRequest { tool, input }`:
```rust
self.pending_permission = Some((tool.clone(), input.clone()));
self.live_region.push_live_block(
    crate::tui::live_region::LiveBlock::PermissionPrompt {
        tool, input,
        menu: crate::tui::permission::PermissionState::new(),
    },
);
```

In the Enter handler for permission (around line 545), after computing `result_text`, replace the block mutation with:
```rust
self.live_region.drop_permission_prompt();
self.live_region.commit_text(&mut self.backend, &result_text)?;
```

Add to `LiveRegion`:
```rust
pub fn drop_permission_prompt(&mut self) {
    self.blocks.retain(|b| !matches!(b, LiveBlock::PermissionPrompt { .. }));
}
```

Adjust `pending_permission` field type in `TerminalUI` to `Option<(String, String)>` and route menu navigation (Up/Down) through the `LiveBlock::PermissionPrompt { menu, .. }` so the rendered menu updates. Add to `LiveRegion`:
```rust
pub fn permission_menu_mut(&mut self) -> Option<&mut crate::tui::permission::PermissionState> {
    for b in self.blocks.iter_mut() {
        if let LiveBlock::PermissionPrompt { menu, .. } = b {
            return Some(menu);
        }
    }
    None
}
pub fn permission_menu(&self) -> Option<&crate::tui::permission::PermissionState> {
    for b in &self.blocks {
        if let LiveBlock::PermissionPrompt { menu, .. } = b {
            return Some(menu);
        }
    }
    None
}
```

- [ ] **Step 5: Add test**

Append to `tests/tui/live_region_test.rs`:
```rust
#[test]
fn finish_last_running_tool_marks_committing_with_output() {
    use viv::tui::tool_call::{ToolCallState, ToolStatus};
    let mut region = LiveRegion::new(TermSize { cols: 40, rows: 10 });
    region.push_live_block(LiveBlock::ToolCall {
        id: 0, name: "Bash".into(), input: "ls".into(),
        output: None, error: None,
        tc_state: ToolCallState::new_running(),
        state: BlockState::Live,
    });
    region.finish_last_running_tool(Some("drwx----".into()), None);
    assert_eq!(region.state_at(0), Some(BlockState::Committing));
}
```

- [ ] **Step 6: Run full suite**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/tui/terminal.rs src/tui/live_region.rs tests/tui/live_region_test.rs
git commit -m "feat(tui): route user/status/tool/permission messages through LiveRegion"
```

---

## Task 10: Replace `TerminalUI::render_frame` with a `LiveRegion::frame` call

**Files:**
- Modify: `src/tui/terminal.rs`

- [ ] **Step 1: Replace body**

Delete the old `render_frame` (lines 643-797) and its `main_layout` / `block_height_with_width` / `render_block` helpers. Replace with:

```rust
fn render_frame(&mut self) -> crate::Result<()> {
    let spinner_frame = if (self.busy && self.spinner_start.is_some()) || self.quitting {
        let elapsed = self.spinner_start.or(self.quitting_start)
            .map(|s| s.elapsed().as_millis() as u64)
            .unwrap_or(0);
        Some(self.spinner.frame_at(elapsed))
    } else { None };
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
    let cur = self.live_region.frame(
        &mut self.backend, &editor, offset, mode, &ctx,
    )?;
    self.backend.move_cursor(cur.row, cur.col)?;
    self.backend.flush()?;
    Ok(())
}
```

- [ ] **Step 2: Wire welcome on Ready**

In `AgentMessage::Ready { model }`:
```rust
self.model_name = model.clone();
// Print welcome once into scrollback.
let welcome_text = crate::tui::welcome::WelcomeWidget::new(
    Some(&model), &self.cwd, self.branch.as_deref(),
).as_scrollback_string();
self.backend.write(welcome_text.as_bytes())?;
self.backend.flush()?;
```

Add `WelcomeWidget::as_scrollback_string()` that renders the widget into a `Buffer` and serializes via `buffer_rows_to_ansi`.

- [ ] **Step 3: Remove the `blocks: Vec<ContentBlock>` field and related code**

Delete from `TerminalUI`: `blocks`, `parse_buffer` (kept — still used), `conversation_state`, `tool_states`, `focus`, `tool_seq` (still used), `welcome_anim`, `selection_state`, and their initializations. Delete `block_height`, `WelcomeAnimState`, and all references.

Keep `parse_buffer`, `tool_seq`.

- [ ] **Step 4: Update the main loop's `Event::Resize` arm**

```rust
Event::Resize(new_size) => {
    self.renderer.resize(new_size);
    self.live_region.resize(new_size);
    dirty = true;
}
```

- [ ] **Step 5: Run full suite**

Run: `cargo test`
Expected: all pass. `tests/tui/terminal_test.rs` may need updates if it asserts on removed fields — remove those assertions.

- [ ] **Step 6: Manual smoke test**

Run: `cargo run`
Expected: welcome prints once; questions/answers flow into scrollback; you can scroll the terminal natively; on exit, full conversation stays visible.

- [ ] **Step 7: Commit**

```bash
git add src/tui/terminal.rs src/tui/welcome.rs src/tui/live_region.rs tests/tui/terminal_test.rs
git commit -m "refactor(tui): render_frame delegates to LiveRegion::frame"
```

---

## Task 11: Remove scroll keys, mouse handling, mouse tracking

**Files:**
- Modify: `src/tui/terminal.rs`
- Modify: `src/core/terminal/events.rs`

- [ ] **Step 1: Strip scroll + mouse from `handle_key` / main loop**

Delete the `KeyEvent::CtrlChar('k')` and `KeyEvent::CtrlChar('j')` arms at the top of `handle_key`.

Delete the four `Event::Mouse(_)` arms in the main loop. Leave the `Event::Mouse(_) => {}` fall-through (or drop the enum variant entirely — see next step).

- [ ] **Step 2: Disable mouse tracking**

In `src/core/terminal/events.rs` and/or `src/core/terminal/backend.rs`, find where mouse tracking is enabled (grep: `1006h`, `1015h`, `1000h`). Remove the enable calls so those escapes are never emitted. Keep the parser as dead code for now; a later PR can delete it.

- [ ] **Step 3: Run full suite**

Run: `cargo test`
Expected: all pass (tests asserting mouse event dispatch should be deleted or updated to expect nothing).

- [ ] **Step 4: Commit**

```bash
git add src/tui/terminal.rs src/core/terminal/events.rs src/core/terminal/backend.rs tests
git commit -m "refactor(tui): drop Ctrl+J/K scroll, mouse events, and tracking"
```

---

## Task 12: Delete dead modules; drop `text_map` from `Renderer`

**Files:**
- Delete: `src/tui/focus.rs`, `src/tui/selection.rs`, `src/tui/text_map.rs`, `src/tui/conversation.rs`
- Delete: `tests/tui/focus_test.rs`, `tests/tui/selection_test.rs`, `tests/tui/text_map_test.rs`, `tests/tui/conversation_test.rs`
- Modify: `src/tui/mod.rs`, `tests/tui/mod.rs`
- Modify: `src/tui/renderer.rs`
- Modify: `tests/tui/renderer_test.rs` (drop text_map assertions)

- [ ] **Step 1: Verify no remaining imports**

Run:
```bash
grep -rn "crate::tui::focus\|crate::tui::selection\|crate::tui::text_map\|crate::tui::conversation\|viv::tui::focus\|viv::tui::selection\|viv::tui::text_map\|viv::tui::conversation" src tests
```
Expected: empty output.

If anything remains, fix it before deleting. Most likely `src/tui/terminal.rs` still imports these — remove the `use` lines.

- [ ] **Step 2: Delete the files**

```bash
git rm src/tui/focus.rs src/tui/selection.rs src/tui/text_map.rs src/tui/conversation.rs
git rm tests/tui/focus_test.rs tests/tui/selection_test.rs tests/tui/text_map_test.rs tests/tui/conversation_test.rs
```

- [ ] **Step 3: Remove module declarations**

Delete `pub mod focus;`, `pub mod selection;`, `pub mod text_map;`, `pub mod conversation;` from `src/tui/mod.rs`.
Delete `mod focus_test;`, `mod selection_test;`, `mod text_map_test;`, `mod conversation_test;` from `tests/tui/mod.rs`.

- [ ] **Step 4: Drop `text_map` from `Renderer`**

In `src/tui/renderer.rs`:
- Remove `use crate::tui::text_map::TextMap;`
- Remove the `text_map: RefCell<TextMap>` field and its initialization
- Remove `text_map()` and `text_map_mut()` methods

In `tests/tui/renderer_test.rs`: delete any test that calls `text_map*()`.

- [ ] **Step 5: Run full suite**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(tui): delete focus/selection/text_map/conversation modules"
```

---

## Task 13: Scripted-conversation integration test

**Files:**
- Create: `tests/tui/inline_flow_test.rs`
- Modify: `tests/tui/mod.rs`

- [ ] **Step 1: Write the test**

Create `tests/tui/inline_flow_test.rs`:

```rust
use viv::agent::protocol::AgentMessage;
use viv::core::terminal::backend::TestBackend;
use viv::tui::input::InputMode;
use viv::tui::live_region::{BlockState, LiveBlock, LiveRegion};
use viv::tui::content::{InlineSpan, MarkdownNode};
use viv::tui::status::StatusContext;
use viv::core::terminal::size::TermSize;

#[test]
fn scripted_flow_produces_scrollback_and_live_region() {
    let mut region = LiveRegion::new(TermSize { cols: 60, rows: 20 });
    let mut backend = TestBackend::new(60, 20);

    // 1. User message commit.
    region.commit_text(&mut backend, "> hello viv").unwrap();

    // 2. Streaming assistant markdown: two closed paragraphs + in-flight.
    for para in ["Sure, here's what I can do.", "Let me check the file."] {
        region.push_live_block(LiveBlock::Markdown {
            nodes: vec![MarkdownNode::Paragraph {
                spans: vec![InlineSpan::Text(para.into())],
            }],
            state: BlockState::Committing,
        });
    }

    // 3. Frame: commits both + paints input/status.
    let ctx = StatusContext {
        cwd: "~/p".into(), branch: Some("main".into()),
        model: "claude-sonnet-4-6".into(),
        input_tokens: 123, output_tokens: 456,
        spinner_frame: None, spinner_verb: String::new(),
    };
    region.frame(&mut backend, "", 0, InputMode::Chat, &ctx).unwrap();

    let out = String::from_utf8(backend.output.clone()).unwrap();
    assert!(out.contains("hello viv"));
    assert!(out.contains("Sure, here's what I can do."));
    assert!(out.contains("Let me check the file."));
    assert!(out.contains("claude-sonnet-4-6"));
    assert_eq!(region.block_count(), 0);
}
```

Add to `tests/tui/mod.rs`: `mod inline_flow_test;`

- [ ] **Step 2: Run**

Run: `cargo test --test tui inline_flow_test`
Expected: pass.

- [ ] **Step 3: Full suite**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add tests/tui/inline_flow_test.rs tests/tui/mod.rs
git commit -m "test(tui): add scripted-conversation integration test for inline flow"
```

---

## Self-review checklist

- [x] Spec coverage: every spec section maps to a task
  - Two-layer storage → Tasks 3-4 (commit path) + Task 5 (live paint)
  - Live region layout → Task 5 (paint)
  - Commit state machine → Tasks 1, 4, 8, 9
  - Removal list → Tasks 11, 12
  - Resize → Task 10 Step 4
  - Startup/exit → Task 7 (alt-screen removed), Task 10 Step 2 (welcome on Ready)
  - Testing → Tasks 1-6, 13
- [x] No `TBD`, no "add appropriate error handling" — every step has real code
- [x] Types consistent: `BlockState` 2-variant across plan; `LiveBlock::Markdown/ToolCall/PermissionPrompt` used consistently; `StatusContext` introduced in Task 5 and referenced thereafter; `CursorPos` used in Tasks 5, 6, 10
- [x] Every task ends with `cargo test` + `git commit`
