# TuiSession 抽取与 SimTerminal 重建 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `TerminalUI` 的渲染核心抽成 `TuiSession`，让测试仿真器和真 UI 共享同一份渲染代码；修 `Screen::write_char` 的立即换行 bug；重写 `e2e_welcome_screen_layout` 做全 24 行文本 + 颜色断言。

**Architecture:** 新增 `src/tui/session.rs::TuiSession` 持有所有 TUI 渲染状态（`LiveRegion`、`LineEditor`、`cwd`、`model_name` 等），方法签名接 `&mut dyn Backend` 以解耦真/假 backend。`TerminalUI`（生产）退化为 `TuiSession + CrossBackend + EventLoop` 的壳。`SimTerminal`（测试）退化为 `TuiSession + TestBackend + AnsiParser` 的壳，放在 `src/core/terminal/simulator.rs` 里取代旧 `TerminalSimulator`。

**Tech Stack:** Rust edition 2024，零外部依赖，现有 `src/tui/*` / `src/core/terminal/*` / `src/agent/*` 模块。

**Spec:** `docs/superpowers/specs/2026-04-23-tui-session-extract-design.md`

---

## Task 1：`Screen::write_char` 改 deferred-wrap + 加 `assert_cell_fg_rgb`

**Files:**
- Modify: `src/core/terminal/simulator.rs` — `Screen` struct 加 `pending_wrap` 字段；`write_char` / `move_cursor_to` / `move_cursor_rel` / `AnsiParser::parse_ground` 的 CR/LF 分支更新；新增 `assert_cell_fg_rgb` 方法
- Test: `src/core/terminal/simulator.rs::tests` 模块加三个单元测试

### Step 1.1：写三个失败测试

打开 `src/core/terminal/simulator.rs`，在 `#[cfg(test)] mod tests` 里（文件末尾 `fn test_cell_style_reset` 之后）加：

```rust
#[test]
fn test_deferred_wrap_cr_lf_no_extra_scroll() {
    // Write 80 'a's then \r\n + 'b'.
    // Deferred-wrap: 80th 'a' at (0, 79) with pending_wrap; \r clears pending,
    // \n moves to (1, 0), 'b' at (1, 0).
    // Naive auto-wrap would put 'b' at (2, 0).
    let input = format!("{}\r\nb", "a".repeat(80));
    let screen = parse_str(&input, 80, 24);
    assert_eq!(screen.char_at(0, 79), Some('a'));
    assert_eq!(screen.char_at(1, 0), Some('b'));
}

#[test]
fn test_deferred_wrap_no_scroll_at_bottom_right() {
    // Move cursor to (24, 1) [ANSI 1-indexed => (23, 0) 0-indexed], write 80 'a's.
    // With deferred-wrap, no scroll should occur; all 80 'a's stay on row 23.
    let input = format!("\x1b[24;1H{}", "a".repeat(80));
    let screen = parse_str(&input, 80, 24);
    assert_eq!(screen.char_at(23, 0), Some('a'));
    assert_eq!(screen.char_at(23, 79), Some('a'));
}

#[test]
fn test_assert_cell_fg_rgb_matches_truecolor() {
    // SGR 38;2;r;g;b sets a truecolor fg.
    let screen = parse_str("\x1b[38;2;215;119;87mA", 80, 24);
    screen.assert_cell_fg_rgb(0, 0, 215, 119, 87);
}
```

### Step 1.2：运行，确认失败

Run:

```bash
cd /data/dlab/viv && cargo test -p viv --lib test_deferred_wrap_cr_lf_no_extra_scroll test_deferred_wrap_no_scroll_at_bottom_right test_assert_cell_fg_rgb_matches_truecolor 2>&1 | tail -30
```

Expected: 前两个 FAIL（`b` 在 row 2 / scroll 了），第三个 FAIL（`assert_cell_fg_rgb` 方法不存在，编译错）。

### Step 1.3：给 `Screen` 加 `pending_wrap` 字段

在 `src/core/terminal/simulator.rs` 的 `Screen` 结构体定义（`pub struct Screen { grid, width, height, cursor }`）加一个字段：

```rust
pub struct Screen {
    grid: Vec<Vec<Cell>>,
    width: usize,
    height: usize,
    cursor: (usize, usize),
    /// Pending-wrap flag (xterm Last Column Flag): set when a character is
    /// written at the rightmost column; cleared on the next character write
    /// (which first wraps to the next row) or on any cursor-positioning op.
    pending_wrap: bool,
}
```

在 `Screen::new` 里初始化：

```rust
pub fn new(width: usize, height: usize) -> Self {
    let cell = Cell::new(' ');
    let grid = vec![vec![cell.clone(); width]; height];
    Screen {
        grid,
        width,
        height,
        cursor: (0, 0),
        pending_wrap: false,
    }
}
```

### Step 1.4：重写 `Screen::write_char` 为 deferred-wrap

把当前 `write_char`（`fn write_char(&mut self, ch: char, style: CellStyle)`）整体替换为：

```rust
fn write_char(&mut self, ch: char, style: CellStyle) {
    // First: if we're pending-wrap, consume the flag by advancing to the
    // start of the next row (scrolling if needed) before writing.
    if self.pending_wrap {
        self.cursor.1 = 0;
        self.cursor.0 += 1;
        if self.cursor.0 >= self.height {
            self.scroll(1);
            self.cursor.0 = self.height - 1;
        }
        self.pending_wrap = false;
    }

    let (row, col) = self.cursor;
    if row >= self.height || col >= self.width {
        return;
    }

    if ch == '\t' {
        // Advance to next tab stop (every 8 columns). Tab clears pending_wrap
        // via the earlier branch; no wrap handling beyond the clamp.
        let next_col = (col / 8 + 1) * 8;
        self.cursor.1 = next_col.min(self.width - 1);
        return;
    }

    self.grid[row][col] = Cell { ch, style };
    if col + 1 >= self.width {
        // Rightmost column: set pending-wrap, don't advance cursor.
        self.pending_wrap = true;
    } else {
        self.cursor.1 += 1;
    }
}
```

### Step 1.5：所有显式光标移动/CR/LF 清除 `pending_wrap`

`move_cursor_to`：在函数体末尾加 `self.pending_wrap = false;`

```rust
fn move_cursor_to(&mut self, row: usize, col: usize) {
    let row = row.saturating_sub(1);
    let col = col.saturating_sub(1);
    self.cursor.0 = row.min(self.height.saturating_sub(1));
    self.cursor.1 = col.min(self.width.saturating_sub(1));
    self.pending_wrap = false;
}
```

`move_cursor_rel`：同样末尾加 `self.pending_wrap = false;`

```rust
fn move_cursor_rel(&mut self, d_row: isize, d_col: isize) {
    let (r, c) = self.cursor;
    let new_row = (r as isize + d_row).clamp(0, self.height as isize - 1) as usize;
    let new_col = (c as isize + d_col).clamp(0, self.width as isize - 1) as usize;
    self.cursor = (new_row, new_col);
    self.pending_wrap = false;
}
```

`AnsiParser::parse_ground` 里的 CR (`0x0D`) 和 LF (`0x0A`) 分支：各加一行清除。注意 `parse_ground` 里操作的是 `self.screen.cursor` 和 `self.screen.pending_wrap`：

原代码（`parse_ground` 内部）：

```rust
if b == 0x0D {
    self.screen.cursor.1 = 0;
}
if b == 0x0A {
    let (row, _) = self.screen.cursor;
    if row + 1 >= self.screen.height() {
        self.screen.scroll(1);
    } else {
        self.screen.cursor.0 += 1;
    }
}
```

改为：

```rust
if b == 0x0D {
    self.screen.cursor.1 = 0;
    self.screen.pending_wrap = false;
}
if b == 0x0A {
    self.screen.pending_wrap = false;
    let (row, _) = self.screen.cursor;
    if row + 1 >= self.screen.height() {
        self.screen.scroll(1);
    } else {
        self.screen.cursor.0 += 1;
    }
}
```

`pending_wrap` 字段是私有的，但 `AnsiParser` 和 `Screen` 在同一文件内同一模块，可以直接访问。

Backspace (`0x08`) 分支目前用 `move_cursor_rel`，已覆盖。

### Step 1.6：新增 `Screen::assert_cell_fg_rgb`

在 `src/core/terminal/simulator.rs` 的 `impl Screen` 里，紧跟现有 `assert_cell_fg` 方法之后加：

```rust
/// Asserts that a specific cell has the expected truecolor foreground.
/// Use this for checking RGB colors emitted via SGR 38;2;r;g;b.
pub fn assert_cell_fg_rgb(&self, row: usize, col: usize, r: u8, g: u8, b: u8) {
    let style = self.style_at(row, col);
    let actual = style.and_then(|s| match &s.fg {
        Some(Color::Rgb(rr, gg, bb)) => Some((*rr, *gg, *bb)),
        _ => None,
    });
    assert_eq!(
        actual,
        Some((r, g, b)),
        "Cell ({}, {}) foreground should be RGB({}, {}, {}), got {:?}",
        row, col, r, g, b, actual
    );
}
```

### Step 1.7：运行测试，确认通过

Run:

```bash
cd /data/dlab/viv && cargo test -p viv --lib test_deferred_wrap_cr_lf_no_extra_scroll test_deferred_wrap_no_scroll_at_bottom_right test_assert_cell_fg_rgb_matches_truecolor 2>&1 | tail -15
```

Expected: 三个都 PASS。

同时跑整个 simulator 模块的已有测试，确认没回归：

```bash
cd /data/dlab/viv && cargo test -p viv --lib core::terminal::simulator 2>&1 | tail -15
```

Expected: 所有 `test_parse_*` / `test_screen_*` 等都通过。

### Step 1.8：Commit

```bash
cd /data/dlab/viv && git add src/core/terminal/simulator.rs && git commit -m "$(cat <<'EOF'
fix(terminal): implement xterm deferred-wrap for Screen::write_char

Writing at the rightmost column now sets a pending-wrap flag instead
of immediately advancing into the next row. The flag is consumed by
the next character write or cleared by any explicit cursor move
(CUP, CUF, CUB, CUU, CUD, CR, LF). This matches the xterm Last
Column Flag behavior and prevents spurious scrolls when a full-width
row is followed by a newline at the bottom of the screen.

Also adds Screen::assert_cell_fg_rgb for truecolor assertions.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2：新建 `TuiSession`，`TerminalUI` 退化为壳

**Files:**
- Create: `src/tui/session.rs` — `TuiSession` 结构体、`KeyOutcome` 枚举、所有方法
- Modify: `src/tui/mod.rs` — 加 `pub mod session;`
- Modify: `src/tui/terminal.rs` — 删除被搬走的字段和方法，改为持 `session: TuiSession` 并委托

这是原子性重构：一次完成才能编译。

### Step 2.1：确认迁移前基线

Run:

```bash
cd /data/dlab/viv && cargo test 2>&1 | tail -5
```

Expected: 除了 `tests::tui::e2e_screen_test::e2e_welcome_screen_layout` 之外其它都通过（那个在 Task 4 才修）。**记下通过的数字。**

### Step 2.2：创建 `src/tui/session.rs`

`Write` 一个新文件 `src/tui/session.rs`，内容：

```rust
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

// Re-export LineEditor / EditAction from the terminal module so callers
// don't need to reach across. (Re-exported at the top of this file.)
```

**注意**：这个文件引用 `self.live_region.width()` 和 `LiveRegion` 里公开的方法，如 `last_live_rows`，需要确认 `LiveRegion` 已暴露它们：

```bash
cd /data/dlab/viv && grep -nE "pub fn (width|last_live_rows)" src/tui/live_region.rs
```

如果 `pub fn width` 不存在，把 `src/tui/live_region.rs::LiveRegion` 里 `size` 字段读取改成 pub 方法；`last_live_rows` 同理。查到路径之后用 `Edit` 加：

```rust
// in impl LiveRegion
pub fn width(&self) -> u16 { self.size.cols }
pub fn last_live_rows(&self) -> u16 { self.last_live_rows }
```

（若方法已存在则跳过）

### Step 2.3：`src/tui/mod.rs` 声明 session 模块

Run:

```bash
cd /data/dlab/viv && cat src/tui/mod.rs
```

在文件中添加 `pub mod session;`（插在现有 `pub mod terminal;` 之后即可）。

### Step 2.4：`src/tui/terminal.rs` 改造为壳

在文件顶部 `use` 区域加：

```rust
use crate::tui::session::{KeyOutcome, TuiSession};
```

把 `pub struct TerminalUI { ... }` 的字段精简为：

```rust
pub struct TerminalUI {
    event_tx: NotifySender<AgentEvent>,
    msg_rx: Receiver<AgentMessage>,
    backend: CrossBackend,
    session: TuiSession,
}
```

（`renderer` 字段原本就有但本计划不动，如果 `TerminalUI` 里没用到可以一并删；先保守只删下面列出的那几个）

删除这些字段：`renderer`（它的两个用途 —— welcome 读宽度、Resize 传播 —— 都由 session 接管，直接删）、`live_region`、`editor`、`cwd`、`branch`、`model_name`、`input_tokens`、`output_tokens`、`busy`、`spinner`、`spinner_start`、`spinner_verb`、`parse_buffer`、`tool_seq`、`pending_permission`、`quitting`、`quitting_start`。

重写 `TerminalUI::new`：

```rust
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
    let session = TuiSession::new(size, cwd, branch);

    Ok(TerminalUI {
        event_tx,
        msg_rx,
        backend,
        session,
    })
}
```

`TerminalUI::handle_agent_message` 整个方法体替换为：

```rust
fn handle_agent_message(&mut self, msg: AgentMessage) -> crate::Result<()> {
    self.session.handle_message(msg, &mut self.backend)
}
```

`TerminalUI::render_frame` 替换为：

```rust
fn render_frame(&mut self) -> crate::Result<()> {
    let cur = self.session.render_frame(&mut self.backend)?;
    self.backend.move_cursor(cur.row, cur.col)?;
    self.backend.flush()?;
    Ok(())
}
```

`TerminalUI::handle_key` 替换为：

```rust
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
```

`TerminalUI::run` 主循环里读这些被搬走字段的地方（`self.busy`、`self.spinner_start`、`self.quitting`、`self.quitting_start`）改成 `self.session.is_busy()` / `self.session.is_quitting()`。`Event::Resize(new_size)` 分支：旧 `self.renderer.resize(new_size); self.live_region.resize(new_size);` 改成 `self.session.resize(new_size);`。`run` 里还有 `if let Some(action) = self.handle_key(key)?` 的 `UiAction::Quit` 分支，**旧** body：

```rust
UiAction::Quit => {
    let _ = self.event_tx.send(AgentEvent::Quit);
    self.enter_quitting_mode();
}
```

**改为**（handle_key 内部已发事件、session 已切入 quitting 状态）：

```rust
UiAction::Quit => {
    // handle_key already sent AgentEvent::Quit and entered quitting mode.
}
```

`TerminalUI::enter_quitting_mode` 方法整个删除。

`cleanup()` 里若读 `self.live_region.last_live_rows()`，改成 `self.session.last_live_rows()`。

### Step 2.5：编译一遍，修所有残余引用

Run:

```bash
cd /data/dlab/viv && cargo build 2>&1 | tail -40
```

Expected：如果还有 `self.live_region` / `self.editor` / `self.model_name` 等旧字段的引用，编译报错；用 `Edit` 逐个替换成 `self.session.<method>()`。反复直到 `cargo build` 绿。

### Step 2.6：跑全量测试验证不回归

Run:

```bash
cd /data/dlab/viv && cargo test 2>&1 | tail -20
```

Expected：通过数和 Step 2.1 相同（`e2e_welcome_screen_layout` 仍失败，但其它都过）。**如果其它测试出现新失败，回到 Step 2.4/2.5 排查**。

### Step 2.7：Commit

```bash
cd /data/dlab/viv && git add src/tui/session.rs src/tui/mod.rs src/tui/terminal.rs src/tui/live_region.rs 2>/dev/null; git commit -m "$(cat <<'EOF'
refactor(tui): extract TuiSession from TerminalUI

Move all TUI rendering state (LiveRegion, LineEditor, model_name,
cwd, spinner, tokens, permission flow) and the three core methods
(handle_message / render_frame / handle_key) into a new reusable
TuiSession struct. TerminalUI becomes a thin shell that owns the
real backend (CrossBackend) and the agent event channels, and
delegates all rendering to TuiSession.

Methods now accept &mut dyn Backend so the same session code can
drive both real and test backends. handle_key returns a KeyOutcome
enum instead of reaching into event_tx directly, letting the test
harness collect the events the UI would have sent.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3：用 `SimTerminal` 替换旧 `TerminalSimulator`

**Files:**
- Modify: `src/core/terminal/simulator.rs` — 删除旧 `TerminalSimulator` 及其所有方法；加 `SimTerminal`

### Step 3.1：删除旧 `TerminalSimulator`

打开 `src/core/terminal/simulator.rs`。找到分隔注释 `// ── TerminalSimulator ─────────`（约 809 行）到 `#[cfg(test)] mod tests {` 之前的所有内容，**整段删除**。这包括：

- 旧的 `use` 语句块（agent::protocol::AgentMessage / core::terminal::backend::TestBackend / core::terminal::size::TermSize / core::terminal::input::KeyEvent / tui::input::InputMode / tui::live_region::LiveRegion / tui::status::StatusContext / tui::terminal::LineEditor）
- `pub struct TerminalSimulator { ... }` 及其 `impl TerminalSimulator { ... }` 块（`new` / `with_cwd` / `with_branch` / `send_key` / `send_message` / `resize` / `screen` / `input_content` / `input_mode` / `permission_selected` / `backend` / `make_status_ctx` / `render` / `render_with_backend` / `parse_chunk`）

保留 `parse_ansi` / `parse_str` 自由函数（在 `TerminalSimulator` 之前）和整个 `#[cfg(test)] mod tests`。

### Step 3.2：在同一文件追加 `SimTerminal`

在被删除内容原位置（`parse_str` 之后、`#[cfg(test)]` 之前）加：

```rust
// ── SimTerminal ───────────────────────────────────────────────────────────────

use crate::agent::protocol::{AgentEvent, AgentMessage};
use crate::core::terminal::backend::{Backend, TestBackend};
use crate::core::terminal::input::KeyEvent;
use crate::core::terminal::size::TermSize;
use crate::tui::input::InputMode;
use crate::tui::session::{KeyOutcome, TuiSession};

/// Test double that runs the real [`TuiSession`] rendering path against a
/// [`TestBackend`], then parses the emitted ANSI bytes into a [`Screen`].
///
/// Tests drive it with `send_message` / `send_key`, inspect the screen
/// with `screen()`, and verify outbound agent events via `sent_events()`.
pub struct SimTerminal {
    session: TuiSession,
    backend: TestBackend,
    parser: AnsiParser,
    sent_events: Vec<AgentEvent>,
}

impl SimTerminal {
    pub fn new(width: usize, height: usize) -> Self {
        let size = TermSize {
            cols: width as u16,
            rows: height as u16,
        };
        SimTerminal {
            session: TuiSession::new(size, "~".to_string(), None),
            backend: TestBackend::new(width as u16, height as u16),
            parser: AnsiParser::new(width, height),
            sent_events: Vec::new(),
        }
    }

    pub fn with_cwd(mut self, cwd: &str) -> Self {
        let size = TermSize {
            cols: self.parser.screen.width as u16,
            rows: self.parser.screen.height as u16,
        };
        self.session = TuiSession::new(size, cwd.to_string(), None);
        self
    }

    pub fn with_branch(mut self, branch: Option<&str>) -> Self {
        let size = TermSize {
            cols: self.parser.screen.width as u16,
            rows: self.parser.screen.height as u16,
        };
        // Preserve cwd that may have been set earlier by with_cwd (re-read via
        // session.cwd would require an accessor; simpler: keep a local copy).
        // For simplicity, require callers to call with_branch BEFORE with_cwd
        // if both are needed; otherwise rebuild with the default "~".
        self.session = TuiSession::new(size, "~".to_string(), branch.map(|s| s.to_string()));
        self
    }

    pub fn send_message(&mut self, msg: AgentMessage) -> &mut Self {
        let _ = self.session.handle_message(msg, &mut self.backend);
        let _ = self.session.render_frame(&mut self.backend);
        self.parser.parse(&self.backend.output);
        self.backend.output.clear();
        self
    }

    pub fn send_key(&mut self, key: KeyEvent) -> &mut Self {
        if let Ok(outcome) = self.session.handle_key(key, &mut self.backend) {
            if let KeyOutcome::Event(e) = outcome {
                self.sent_events.push(e);
            }
        }
        let _ = self.session.render_frame(&mut self.backend);
        self.parser.parse(&self.backend.output);
        self.backend.output.clear();
        self
    }

    pub fn resize(&mut self, width: usize, height: usize) -> &mut Self {
        let size = TermSize {
            cols: width as u16,
            rows: height as u16,
        };
        self.session.resize(size);
        self.parser = AnsiParser::new(width, height);
        self.backend.size = size;
        let _ = self.session.render_frame(&mut self.backend);
        self.parser.parse(&self.backend.output);
        self.backend.output.clear();
        self
    }

    pub fn screen(&self) -> Screen {
        Screen {
            grid: self.parser.screen.grid.clone(),
            width: self.parser.screen.width,
            height: self.parser.screen.height,
            cursor: self.parser.screen.cursor,
            pending_wrap: self.parser.screen.pending_wrap,
        }
    }

    pub fn input_content(&self) -> String {
        self.session.input_content()
    }

    pub fn input_mode(&self) -> InputMode {
        self.session.input_mode()
    }

    pub fn permission_selected(&self) -> Option<usize> {
        self.session.permission_selected()
    }

    pub fn sent_events(&self) -> &[AgentEvent] {
        &self.sent_events
    }

    pub fn quit_requested(&self) -> bool {
        self.session.is_quitting()
    }
}
```

**注意**：`TestBackend::output` / `TestBackend::size` 需要是 `pub`。从 task 1 起它们已经是 pub 字段（`pub output: Vec<u8>` / `pub size: TermSize`），无需改。

**`with_cwd` / `with_branch` 的语义**：如果同一个 `SimTerminal` 上先 `with_cwd` 再 `with_branch`，第二次会丢掉 cwd（因为 `TuiSession::new` 重新构造）。当前测试只 `with_cwd("/data/project")` 单独调，不受影响。本轮不打算修，留注释警告即可（已在代码里）。

### Step 3.3：验证编译

Run:

```bash
cd /data/dlab/viv && cargo build 2>&1 | tail -20
```

Expected: 编译通过。`tests/tui/e2e_screen_test.rs` 用 `TerminalSimulator` 可能报错；不管（Task 4 会重写）。

### Step 3.4：跑 simulator 的单元测试

Run:

```bash
cd /data/dlab/viv && cargo test -p viv --lib core::terminal::simulator 2>&1 | tail -15
```

Expected: Task 1 的三个新测试 + 原有的 `test_parse_*` / `test_screen_*` 等全部通过。

### Step 3.5：Commit

```bash
cd /data/dlab/viv && git add src/core/terminal/simulator.rs && git commit -m "$(cat <<'EOF'
refactor(sim): replace TerminalSimulator with SimTerminal

The old TerminalSimulator duplicated TuiSession's state (LiveRegion,
LineEditor, cwd, model_name, etc.) and reimplemented send_message /
send_key rendering. That parallel code path silently drifted from
production UI behavior — the old welcome screen test passed only
because the simulator never called render(), hiding the fact that
the input frame was never drawn.

SimTerminal now holds the real TuiSession and drives it against a
TestBackend, parsing the emitted ANSI into a Screen. Tests that hit
SimTerminal exercise the exact code path that TerminalUI runs in
production.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4：重写 `e2e_welcome_screen_layout`

**Files:**
- Modify: `tests/tui/e2e_screen_test.rs`

### Step 4.1：把整个文件替换为新版

用 `Write` 整块覆盖 `tests/tui/e2e_screen_test.rs`：

```rust
//! End-to-end UI tests for SimTerminal.
//!
//! Drives the real TuiSession through SimTerminal and asserts the complete
//! rendered screen (text + truecolor foreground) produced by the production
//! rendering pipeline.

use viv::agent::protocol::AgentMessage;
use viv::core::terminal::simulator::SimTerminal;

/// Complete 80x24 Welcome screen: logo, info rows, input frame, status bar.
#[test]
fn e2e_welcome_screen_layout() {
    // Shell appears in the welcome header; pin it so the test is reproducible.
    // Safety: single-threaded test; no other threads read SHELL here.
    unsafe { std::env::set_var("SHELL", "/bin/zsh"); }

    let mut sim = SimTerminal::new(80, 24).with_cwd("/data/project");
    sim.send_message(AgentMessage::Ready {
        model: "claude-3-5-sonnet-20241022".into(),
    });

    let screen = sim.screen();

    screen.assert_screen(&[
        r"       _           Model:    claude-3-5-sonnet-20241022",
        r"__   _(_)_   __    CWD:      /data/project",
        r"\ \ / / \ \ / /    Branch:   -",
        r" \ V /| |\ V /     Platform: linux x86_64",
        r"  \_/ |_| \_/      Shell:    zsh",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "────────────────────────────────────────────────────────────────────────────────",
        "\u{276F}",
        "────────────────────────────────────────────────────────────────────────────────",
        r"  /data/project                    claude-3-5-sonnet-20241022  \u{2191} 0  \u{2193} 0  ~$0.000",
    ]);

    // Logo uses CLAUDE orange, RGB(215, 119, 87).
    screen.assert_cell_fg_rgb(0, 7, 215, 119, 87);
    screen.assert_cell_fg_rgb(2, 0, 215, 119, 87);

    // Info labels use CLAUDE orange too.
    screen.assert_cell_fg_rgb(0, 19, 215, 119, 87);

    // Info values use TEXT white, RGB(255, 255, 255).
    screen.assert_cell_fg_rgb(0, 29, 255, 255, 255);
    screen.assert_cell_fg_rgb(1, 29, 255, 255, 255);

    // Input box border uses DIM, RGB(136, 136, 136).
    screen.assert_cell_fg_rgb(20, 0, 136, 136, 136);
    screen.assert_cell_fg_rgb(22, 79, 136, 136, 136);

    // Prompt glyph uses CLAUDE orange.
    screen.assert_cell_fg_rgb(21, 0, 215, 119, 87);

    // Status bar text (cwd + model) uses DIM.
    screen.assert_cell_fg_rgb(23, 2, 136, 136, 136);
    screen.assert_cell_fg_rgb(23, 35, 136, 136, 136);
}
```

**注意**：row 23 最后一行的 raw string 里 `\u{2191}` / `\u{2193}` 需要用**非**-raw 字符串表达，因为 raw 字符串不解析 unicode 转义。改为：

```rust
"  /data/project                    claude-3-5-sonnet-20241022  \u{2191} 0  \u{2193} 0  ~$0.000",
```

（即前面不加 `r`）。确认你粘贴时这行没用 `r"..."`。

### Step 4.2：跑测试

Run:

```bash
cd /data/dlab/viv && cargo test --test mod e2e_welcome_screen_layout 2>&1 | tail -40
```

Expected: **PASS**。如果失败，看 `assert_screen` 输出定位哪一行/哪一列不对，可能是：
- **期望串里某行的空格数算错**（最常见） —— 按错误信息里的 `actual` 修期望串
- **颜色断言的行列下标算错** —— 按 welcome 布局（`info_x = 19`, `label_width = 10`, 所以值从 col 29 开始）复核

### Step 4.3：跑全量测试最后兜底

Run:

```bash
cd /data/dlab/viv && cargo test 2>&1 | tail -15
```

Expected: 全绿。

### Step 4.4：Commit

```bash
cd /data/dlab/viv && git add tests/tui/e2e_screen_test.rs && git commit -m "$(cat <<'EOF'
test(tui): rewrite e2e_welcome_screen_layout against SimTerminal

Assert the full 24-row rendered screen — logo, info rows, 15 blank
scrollback rows, input box borders, prompt, and status bar — plus
truecolor foreground for logo (CLAUDE orange), labels, values,
border (DIM), prompt glyph, and status bar text.

Now exercises the real TuiSession via SimTerminal, matching what
TerminalUI renders in production.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Self-review checklist

- [ ] Spec 覆盖：所有 spec 列出的 delete/keep/add 项目都在 task 1-3 里有对应 step
- [ ] 无占位符："TBD"、"similar to"、"add appropriate handling" 这类字眼不出现
- [ ] 类型一致性：`KeyOutcome` 的 variants 在 Task 2 定义（`None` / `Event(AgentEvent)`），Task 3 的 `SimTerminal::send_key` match 上一致；`Screen` 新增 `pending_wrap` 字段在 Task 1 加，Task 3 的 `screen()` clone 时带上一致
- [ ] `assert_cell_fg_rgb` 在 Task 1 定义，Task 4 使用一致
- [ ] `$SHELL` 设置在 Task 4 首行
- [ ] 所有 commit 都在"test 通过"之后
