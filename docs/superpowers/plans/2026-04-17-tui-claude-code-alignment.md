# TUI Claude Code Alignment — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align viv's TUI with Claude Code's visual and interaction style across six modules: message style, header, status bar, multiline input, TUI-integrated permission prompts, and Markdown rendering.

**Architecture:** Modular Widget additions under `src/tui/`, each independently testable. Layout in `repl.rs` gains a Header row and a Status row; the old footer is replaced. StreamResult gains token counts fed into AgentContext and rendered in the status bar.

**Tech Stack:** Rust (std-only), existing `tui::Widget` trait, `terminal::buffer::{Buffer, Rect}`, `terminal::style::theme`.

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `src/tui/message_style.rs` | Modify | User `>` → orange; `format_welcome` accepts cwd+branch |
| `src/tui/header.rs` | Create | Header widget (cwd + git branch) |
| `src/tui/status.rs` | Create | Status bar widget (model + tokens + cost) |
| `src/tui/input.rs` | Modify | Add `placeholder` field, multiline render |
| `src/tui/permission.rs` | Create | `render_permission_pending` / `render_permission_result` helpers |
| `src/tui/markdown.rs` | Create | MVP Markdown → `Vec<Line>` |
| `src/tui/mod.rs` | Modify | `pub mod` for 3 new files |
| `src/llm.rs` | Modify | `StreamResult` gains `input_tokens`, `output_tokens` |
| `src/agent/context.rs` | Modify | `AgentContext` gains cumulative token fields |
| `src/agent/run.rs` | Modify | Accumulate tokens from `stream_result` |
| `src/repl.rs` | Modify | 4-row layout, multiline `LineEditor`, TUI ask_fn, welcome with cwd |
| `tests/tui/mod.rs` | Modify | Register 3 new test modules |
| `tests/tui/message_style_test.rs` | Modify | Update user `>` color + welcome assertions |
| `tests/tui/header_test.rs` | Create | Header render + git branch parsing |
| `tests/tui/status_test.rs` | Create | Status render + cost calculation |
| `tests/tui/input_test.rs` | Modify | Placeholder + multiline render |
| `tests/tui/permission_test.rs` | Create | Permission line render |
| `tests/tui/markdown_test.rs` | Create | Markdown → Line conversion |
| `tests/repl_test.rs` | Modify | Multiline `LineEditor` key handling |

---

## Task 1: User `>` Color — Orange Instead of Dim

**Files:**
- Modify: `src/tui/message_style.rs:11-16`
- Modify: `tests/tui/message_style_test.rs:6-12`

- [ ] **Step 1: Update the failing assertion — change expected color in test**

In `tests/tui/message_style_test.rs` replace:
```rust
assert_eq!(line.spans[0].fg, Some(theme::DIM));
```
with:
```rust
assert_eq!(line.spans[0].fg, Some(theme::CLAUDE));
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test --test tui_tests message_style 2>&1 | grep -E "FAILED|error"
```
Expected: FAILED — `Some(Rgb(136, 136, 136))` ≠ `Some(Rgb(215, 119, 87))`

- [ ] **Step 3: Change `format_user_message` to use `theme::CLAUDE`**

In `src/tui/message_style.rs:12`:
```rust
pub fn format_user_message(text: &str) -> Line {
    Line::from_spans(vec![
        Span::styled("> ", theme::CLAUDE, false),
        Span::raw(text),
    ])
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test --test tui_tests message_style
```
Expected: all `message_style` tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/tui/message_style.rs tests/tui/message_style_test.rs
git commit -m "feat(tui): user message > prefix color → Claude orange"
```

---

## Task 2: Header Widget — cwd + git Branch

**Files:**
- Create: `src/tui/header.rs`
- Create: `tests/tui/header_test.rs`
- Modify: `src/tui/mod.rs`
- Modify: `tests/tui/mod.rs`

- [ ] **Step 1: Write failing tests in `tests/tui/header_test.rs`**

```rust
use viv::tui::header::HeaderWidget;
use viv::tui::widget::Widget;
use viv::terminal::buffer::{Buffer, Rect};
use viv::terminal::style::theme;

#[test]
fn renders_cwd_without_branch() {
    let w = HeaderWidget { cwd: "~/project".to_string(), branch: None };
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
    w.render(Rect::new(0, 0, 40, 1), &mut buf);
    // Check '~' appears at col 2 (two leading spaces)
    assert_eq!(buf.get(2, 0).ch, '~');
}

#[test]
fn renders_branch_when_present() {
    let w = HeaderWidget { cwd: "~/p".to_string(), branch: Some("main".to_string()) };
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
    w.render(Rect::new(0, 0, 40, 1), &mut buf);
    // Text should contain ⎇
    let rendered: String = (0..40).map(|x| buf.get(x, 0).ch).collect();
    assert!(rendered.contains('⎇'), "should contain branch symbol");
}

#[test]
fn truncates_long_cwd() {
    let long = "~/very/long/path/that/exceeds/thirty/chars/yes";
    let w = HeaderWidget::from_path(long, None);
    assert!(w.cwd.chars().count() <= 32); // "…" + 29 chars
}

#[test]
fn text_is_dim() {
    let w = HeaderWidget { cwd: "~/p".to_string(), branch: None };
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
    w.render(Rect::new(0, 0, 20, 1), &mut buf);
    assert_eq!(buf.get(2, 0).fg, Some(theme::DIM));
}

#[test]
fn parse_git_branch_from_head_content() {
    let content = "ref: refs/heads/my-feature\n";
    assert_eq!(
        viv::tui::header::parse_branch(content),
        Some("my-feature".to_string())
    );
}

#[test]
fn parse_git_branch_detached_head_returns_none() {
    let content = "abc1234567890abcdef1234567890abcdef12345\n";
    assert_eq!(viv::tui::header::parse_branch(content), None);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --test tui_tests header 2>&1 | grep -E "error|FAILED"
```
Expected: compile error — `header` module not found.

- [ ] **Step 3: Create `src/tui/header.rs`**

```rust
use crate::terminal::buffer::{Buffer, Rect};
use crate::terminal::style::theme;
use crate::tui::widget::Widget;

pub struct HeaderWidget {
    pub cwd: String,
    pub branch: Option<String>,
}

impl HeaderWidget {
    pub fn from_env() -> Self {
        let raw_cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "?".to_string());
        let home = std::env::var("HOME").unwrap_or_default();
        let cwd = if !home.is_empty() && raw_cwd.starts_with(&home) {
            format!("~{}", &raw_cwd[home.len()..])
        } else {
            raw_cwd
        };
        let branch = std::fs::read_to_string(".git/HEAD")
            .ok()
            .and_then(|s| parse_branch(&s));
        Self::from_path(&cwd, branch)
    }

    pub fn from_path(cwd: &str, branch: Option<String>) -> Self {
        let cwd = if cwd.chars().count() > 30 {
            let tail: String = cwd.chars().rev().take(29).collect::<String>()
                .chars().rev().collect();
            format!("…{}", tail)
        } else {
            cwd.to_string()
        };
        HeaderWidget { cwd, branch }
    }
}

pub fn parse_branch(head_content: &str) -> Option<String> {
    head_content
        .trim()
        .strip_prefix("ref: refs/heads/")
        .map(|b| b.to_string())
}

impl Widget for HeaderWidget {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() { return; }
        let text = match &self.branch {
            Some(b) => format!("  {}  ⎇ {}", self.cwd, b),
            None => format!("  {}", self.cwd),
        };
        buf.set_str(area.x, area.y, &text, Some(theme::DIM), false);
    }
}
```

- [ ] **Step 4: Register module in `src/tui/mod.rs`**

Add `pub mod header;` to the file:
```rust
pub mod block;
pub mod header;
pub mod input;
pub mod layout;
pub mod message_style;
pub mod paragraph;
pub mod renderer;
pub mod spinner;
pub mod widget;
```

- [ ] **Step 5: Register test module in `tests/tui/mod.rs`**

Add `mod header_test;` to `tests/tui/mod.rs`:
```rust
mod block_test;
mod header_test;
mod input_test;
mod layout_test;
mod message_style_test;
mod paragraph_test;
mod renderer_test;
mod spinner_test;
mod widget_test;
```

- [ ] **Step 6: Run tests to verify they pass**

```bash
cargo test --test tui_tests header
```
Expected: 6 tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/tui/header.rs src/tui/mod.rs tests/tui/header_test.rs tests/tui/mod.rs
git commit -m "feat(tui): add HeaderWidget with cwd and git branch display"
```

---

## Task 3: Status Bar Widget — Model + Tokens + Cost

**Files:**
- Create: `src/tui/status.rs`
- Create: `tests/tui/status_test.rs`
- Modify: `src/tui/mod.rs`
- Modify: `tests/tui/mod.rs`

- [ ] **Step 1: Write failing tests in `tests/tui/status_test.rs`**

```rust
use viv::tui::status::StatusWidget;
use viv::tui::widget::Widget;
use viv::terminal::buffer::{Buffer, Rect};
use viv::terminal::style::theme;

#[test]
fn renders_model_name() {
    let w = StatusWidget {
        model: "claude-sonnet-4-6".to_string(),
        input_tokens: 0,
        output_tokens: 0,
    };
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 1));
    w.render(Rect::new(0, 0, 60, 1), &mut buf);
    let rendered: String = (0..60).map(|x| buf.get(x, 0).ch).collect();
    assert!(rendered.contains("claude-sonnet-4-6"), "model name should appear");
}

#[test]
fn renders_token_counts() {
    let w = StatusWidget {
        model: "m".to_string(),
        input_tokens: 1000,
        output_tokens: 250,
    };
    let mut buf = Buffer::empty(Rect::new(0, 0, 60, 1));
    w.render(Rect::new(0, 0, 60, 1), &mut buf);
    let rendered: String = (0..60).map(|x| buf.get(x, 0).ch).collect();
    assert!(rendered.contains("1000"), "input tokens");
    assert!(rendered.contains("250"), "output tokens");
}

#[test]
fn cost_calculation_sonnet_pricing() {
    let w = StatusWidget {
        model: "m".to_string(),
        input_tokens: 1_000_000,
        output_tokens: 1_000_000,
    };
    // Sonnet: $3/M input + $15/M output = $18 total
    let cost = w.estimate_cost();
    assert!((cost - 18.0).abs() < 0.001, "expected $18.000, got {}", cost);
}

#[test]
fn zero_tokens_shows_zero_cost() {
    let w = StatusWidget { model: "m".to_string(), input_tokens: 0, output_tokens: 0 };
    assert_eq!(w.estimate_cost(), 0.0);
}

#[test]
fn text_is_dim() {
    let w = StatusWidget { model: "m".to_string(), input_tokens: 0, output_tokens: 0 };
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
    w.render(Rect::new(0, 0, 40, 1), &mut buf);
    // First non-space cell should be dim
    assert_eq!(buf.get(2, 0).fg, Some(theme::DIM));
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --test tui_tests status 2>&1 | grep -E "error|FAILED"
```
Expected: compile error — `status` module not found.

- [ ] **Step 3: Create `src/tui/status.rs`**

```rust
use crate::terminal::buffer::{Buffer, Rect};
use crate::terminal::style::theme;
use crate::tui::widget::Widget;

// Anthropic claude-sonnet-4-6 pricing (USD per million tokens, as of 2026-04)
const INPUT_PRICE_PER_M: f64 = 3.0;
const OUTPUT_PRICE_PER_M: f64 = 15.0;

pub struct StatusWidget {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl StatusWidget {
    pub fn estimate_cost(&self) -> f64 {
        (self.input_tokens as f64 / 1_000_000.0) * INPUT_PRICE_PER_M
            + (self.output_tokens as f64 / 1_000_000.0) * OUTPUT_PRICE_PER_M
    }
}

impl Widget for StatusWidget {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() { return; }
        let cost = self.estimate_cost();
        let text = format!(
            "  {}  ↑ {}  ↓ {}  ~${:.3}",
            self.model, self.input_tokens, self.output_tokens, cost
        );
        buf.set_str(area.x, area.y, &text, Some(theme::DIM), false);
    }
}
```

- [ ] **Step 4: Register module in `src/tui/mod.rs` and `tests/tui/mod.rs`**

`src/tui/mod.rs`:
```rust
pub mod block;
pub mod header;
pub mod input;
pub mod layout;
pub mod message_style;
pub mod paragraph;
pub mod renderer;
pub mod spinner;
pub mod status;
pub mod widget;
```

`tests/tui/mod.rs`:
```rust
mod block_test;
mod header_test;
mod input_test;
mod layout_test;
mod message_style_test;
mod paragraph_test;
mod renderer_test;
mod spinner_test;
mod status_test;
mod widget_test;
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cargo test --test tui_tests status
```
Expected: 5 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/tui/status.rs src/tui/mod.rs tests/tui/status_test.rs tests/tui/mod.rs
git commit -m "feat(tui): add StatusWidget with model, token counts, and cost"
```

---

## Task 4: Token Tracking — StreamResult + AgentContext

**Files:**
- Modify: `src/llm.rs` (StreamResult struct + parse_agent_stream)
- Modify: `src/agent/context.rs`
- Modify: `src/agent/run.rs`

- [ ] **Step 1: Add token fields to `StreamResult` in `src/llm.rs`**

Find `pub struct StreamResult` (line ~311) and replace:
```rust
pub struct StreamResult {
    pub text_blocks: Vec<crate::agent::message::ContentBlock>,
    pub tool_uses: Vec<crate::agent::message::ContentBlock>,
    pub stop_reason: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}
```

Update all three `StreamResult { ... }` construction sites (lines ~436, ~507) to include the new fields.

In `parse_agent_stream`, add accumulators after `let mut stop_reason = ...`:
```rust
let mut input_tokens: u64 = 0;
let mut output_tokens: u64 = 0;
```

And in the `match ev_type` block, handle `"message_start"` and `"message_delta"`:
```rust
"message_start" => {
    if let Some(usage) = json.get("message").and_then(|m| m.get("usage")) {
        input_tokens += usage.get("input_tokens")
            .and_then(|v| v.as_i64()).unwrap_or(0) as u64;
    }
}
"message_delta" => {
    if let Some(reason) = json.get("delta")
        .and_then(|d| d.get("stop_reason"))
        .and_then(|v| v.as_str())
    {
        stop_reason = reason.to_string();
    }
    if let Some(usage) = json.get("usage") {
        output_tokens += usage.get("output_tokens")
            .and_then(|v| v.as_i64()).unwrap_or(0) as u64;
    }
}
```

Update the final `Ok(StreamResult { ... })` to include the new fields:
```rust
Ok(StreamResult { text_blocks, tool_uses, stop_reason, input_tokens, output_tokens })
```

Also update the early-return `StreamResult` at line ~436:
```rust
None => return Ok(StreamResult {
    text_blocks, tool_uses, stop_reason,
    input_tokens: 0, output_tokens: 0
}),
```

- [ ] **Step 2: Run existing tests to confirm nothing broke**

```bash
cargo test --test tui_tests && cargo test --test agent_tests 2>&1 | grep -E "FAILED|error\[" | head -20
```
Expected: all tests pass (no changes to test assertions yet).

- [ ] **Step 3: Add token fields to `AgentContext` in `src/agent/context.rs`**

Add two fields to `AgentContext`:
```rust
pub struct AgentContext {
    pub messages: Vec<Message>,
    pub prompt_cache: PromptCache,
    pub llm: Arc<LLMClient>,
    pub store: Arc<MemoryStore>,
    pub index: Arc<Mutex<MemoryIndex>>,
    pub config: AgentConfig,
    pub tool_registry: ToolRegistry,
    pub permission_manager: PermissionManager,
    pub input_tokens: u64,
    pub output_tokens: u64,
}
```

Update `AgentContext::new` to initialize them:
```rust
Ok(AgentContext {
    messages: vec![],
    prompt_cache: PromptCache::default(),
    llm,
    store,
    index,
    config: AgentConfig::default(),
    tool_registry,
    permission_manager: PermissionManager::default(),
    input_tokens: 0,
    output_tokens: 0,
})
```

- [ ] **Step 4: Accumulate tokens in `run_agent` in `src/agent/run.rs`**

After `let stream_result = ctx.llm.stream_agent(...)? ;` (line ~52), add:
```rust
ctx.input_tokens += stream_result.input_tokens;
ctx.output_tokens += stream_result.output_tokens;
```

- [ ] **Step 5: Run all tests**

```bash
cargo test 2>&1 | grep -E "FAILED|error\[" | head -20
```
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/llm.rs src/agent/context.rs src/agent/run.rs
git commit -m "feat(llm): track input/output tokens in StreamResult and AgentContext"
```

---

## Task 5: Input Placeholder

**Files:**
- Modify: `src/tui/input.rs`
- Modify: `tests/tui/input_test.rs`

- [ ] **Step 1: Write failing tests — add to `tests/tui/input_test.rs`**

Append at end of file:
```rust
#[test]
fn placeholder_shown_when_content_empty() {
    let w = InputWidget::new("", 0, "> ")
        .placeholder(Some("How can I help you?"));
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 1));
    w.render(Rect::new(0, 0, 30, 1), &mut buf);
    // After the prompt (col 2), placeholder text starts
    assert_eq!(buf.get(2, 0).ch, 'H');
}

#[test]
fn placeholder_hidden_when_content_present() {
    let w = InputWidget::new("x", 1, "> ")
        .placeholder(Some("How can I help you?"));
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 1));
    w.render(Rect::new(0, 0, 30, 1), &mut buf);
    assert_eq!(buf.get(2, 0).ch, 'x');
}

#[test]
fn placeholder_is_dim_colored() {
    let w = InputWidget::new("", 0, "> ")
        .placeholder(Some("hint"));
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
    w.render(Rect::new(0, 0, 20, 1), &mut buf);
    assert_eq!(buf.get(2, 0).fg, Some(viv::terminal::style::theme::DIM));
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --test tui_tests input 2>&1 | grep -E "FAILED|error\["
```
Expected: compile error — no `placeholder` method.

- [ ] **Step 3: Add `placeholder` field and builder method to `InputWidget` in `src/tui/input.rs`**

Update the struct and `new`:
```rust
pub struct InputWidget<'a> {
    pub content: &'a str,
    pub cursor: usize,
    pub prompt: &'a str,
    pub prompt_fg: Option<Color>,
    pub placeholder: Option<&'a str>,
}

impl<'a> InputWidget<'a> {
    pub fn new(content: &'a str, cursor: usize, prompt: &'a str) -> Self {
        InputWidget { content, cursor, prompt, prompt_fg: None, placeholder: None }
    }

    pub fn prompt_fg(mut self, fg: Color) -> Self {
        self.prompt_fg = Some(fg);
        self
    }

    pub fn placeholder(mut self, text: Option<&'a str>) -> Self {
        self.placeholder = text;
        self
    }
```

In the `render` impl, before the character rendering loop, add the placeholder branch:
```rust
impl Widget for InputWidget<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() { return; }

        // If content is empty and placeholder is set, render prompt + placeholder
        if self.content.is_empty() {
            if let Some(ph) = self.placeholder {
                let scroll = 0usize;
                let available = area.width;
                let prompt_chars: Vec<char> = self.prompt.chars().collect();
                let ph_chars: Vec<char> = ph.chars().collect();
                let mut col = area.x;
                let mut logical_col = 0usize;
                for (ch, is_prompt) in prompt_chars.iter().map(|&c| (c, true))
                    .chain(ph_chars.iter().map(|&c| (c, false)))
                {
                    let w = char_width(ch) as usize;
                    if w == 0 { continue; }
                    if logical_col + w <= scroll { logical_col += w; continue; }
                    if col >= area.x + available { break; }
                    let fg = if is_prompt { self.prompt_fg } else { Some(crate::terminal::style::theme::DIM) };
                    let cell = buf.get_mut(col, area.y);
                    cell.ch = ch;
                    cell.fg = fg;
                    cell.bold = false;
                    col += w as u16;
                    logical_col += w;
                }
                return;
            }
        }

        // existing rendering code follows unchanged ...
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --test tui_tests input
```
Expected: all input tests pass (including the 3 new ones).

- [ ] **Step 5: Commit**

```bash
git add src/tui/input.rs tests/tui/input_test.rs
git commit -m "feat(tui): add placeholder support to InputWidget"
```

---

## Task 6: Multiline LineEditor + InputWidget Multiline Render

**Files:**
- Modify: `src/repl.rs` (LineEditor struct + handle_key)
- Modify: `src/tui/input.rs` (multiline render + cursor_position)
- Modify: `tests/repl_test.rs`
- Modify: `tests/tui/input_test.rs`

- [ ] **Step 1: Write failing tests for multiline LineEditor**

In `tests/repl_test.rs`, add:
```rust
use viv::repl::{LineEditor, EditAction};
use viv::terminal::input::KeyEvent;

#[test]
fn shift_enter_inserts_new_line() {
    let mut ed = LineEditor::new();
    ed.handle_key(KeyEvent::Char('a'));
    ed.handle_key(KeyEvent::ShiftEnter);
    ed.handle_key(KeyEvent::Char('b'));
    assert_eq!(ed.lines, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(ed.row, 1);
    assert_eq!(ed.col, 1);
}

#[test]
fn enter_submits_all_lines_joined() {
    let mut ed = LineEditor::new();
    ed.handle_key(KeyEvent::Char('a'));
    ed.handle_key(KeyEvent::ShiftEnter);
    ed.handle_key(KeyEvent::Char('b'));
    let action = ed.handle_key(KeyEvent::Enter);
    assert_eq!(action, EditAction::Submit("a\nb".to_string()));
    assert_eq!(ed.lines, vec!["".to_string()]);
    assert_eq!(ed.row, 0);
    assert_eq!(ed.col, 0);
}

#[test]
fn backspace_at_line_start_merges_with_previous() {
    let mut ed = LineEditor::new();
    ed.handle_key(KeyEvent::Char('a'));
    ed.handle_key(KeyEvent::ShiftEnter);
    ed.handle_key(KeyEvent::Backspace); // col=0, merge
    assert_eq!(ed.lines, vec!["a".to_string()]);
    assert_eq!(ed.row, 0);
    assert_eq!(ed.col, 1);
}

#[test]
fn cursor_offset_in_multiline() {
    let mut ed = LineEditor::new();
    ed.handle_key(KeyEvent::Char('a'));
    ed.handle_key(KeyEvent::Char('b'));
    ed.handle_key(KeyEvent::ShiftEnter);
    ed.handle_key(KeyEvent::Char('c'));
    // "ab\nc", cursor at 'c' end → offset = 2 (ab) + 1 (\n) + 1 (c) = 4
    assert_eq!(ed.cursor_offset(), 4);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --test repl_test 2>&1 | grep -E "error\[|FAILED"
```
Expected: compile errors — `lines`, `row`, `col` fields don't exist yet.

- [ ] **Step 3: Replace `LineEditor` in `src/repl.rs` with multiline version**

Replace the entire `LineEditor` struct and impl (lines 385–480):

```rust
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
            KeyEvent::Home => { self.col = 0; EditAction::Continue }
            KeyEvent::End => {
                self.col = self.lines[self.row].len();
                EditAction::Continue
            }
            KeyEvent::CtrlC => EditAction::Interrupt,
            KeyEvent::CtrlD => {
                if self.is_empty() { EditAction::Exit } else { EditAction::Continue }
            }
            _ => EditAction::Continue,
        }
    }

    fn prev_char_boundary(&self) -> usize {
        let mut pos = self.col.saturating_sub(1);
        while pos > 0 && !self.lines[self.row].is_char_boundary(pos) { pos -= 1; }
        pos
    }

    fn next_char_boundary(&self) -> usize {
        let line = &self.lines[self.row];
        let mut pos = self.col + 1;
        while pos < line.len() && !line.is_char_boundary(pos) { pos += 1; }
        pos
    }
}

impl Default for LineEditor {
    fn default() -> Self { Self::new() }
}
```

- [ ] **Step 4: Update `InputWidget` for multiline content in `src/tui/input.rs`**

`InputWidget` already accepts `content: &str`. Extend `render` and `cursor_position` to handle `\n` in content. Replace the `cursor_position` method:

```rust
pub fn cursor_position(&self, area: Rect) -> (u16, u16) {
    let prompt_width: u16 = self.prompt.chars().map(char_width).sum();
    let before = &self.content[..self.cursor.min(self.content.len())];
    let cursor_row = before.chars().filter(|&c| c == '\n').count() as u16;
    let last_nl = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let cursor_col: u16 = before[last_nl..].chars().map(char_width).sum();
    let col = area.x + prompt_width + cursor_col;
    let row = area.y + cursor_row;
    (col, row)
}
```

Replace the `render` impl body to handle multiline:
```rust
fn render(&self, area: Rect, buf: &mut Buffer) {
    if area.is_empty() { return; }

    // Placeholder branch (content empty)
    if self.content.is_empty() {
        if let Some(ph) = self.placeholder {
            let mut col = area.x;
            for (ch, is_prompt) in self.prompt.chars().map(|c| (c, true))
                .chain(ph.chars().map(|c| (c, false)))
            {
                let w = char_width(ch) as usize;
                if w == 0 { continue; }
                if col + w as u16 > area.x + area.width { break; }
                let fg = if is_prompt {
                    self.prompt_fg
                } else {
                    Some(crate::terminal::style::theme::DIM)
                };
                let cell = buf.get_mut(col, area.y);
                cell.ch = ch; cell.fg = fg; cell.bold = false;
                col += w as u16;
            }
        } else {
            // Just render the prompt
            let mut col = area.x;
            for ch in self.prompt.chars() {
                let w = char_width(ch);
                if col + w > area.x + area.width { break; }
                let cell = buf.get_mut(col, area.y);
                cell.ch = ch; cell.fg = self.prompt_fg; cell.bold = false;
                col += w;
            }
        }
        return;
    }

    let prompt_width: u16 = self.prompt.chars().map(char_width).sum();
    let logical_lines: Vec<&str> = self.content.split('\n').collect();

    for (row_idx, line) in logical_lines.iter().enumerate() {
        let y = area.y + row_idx as u16;
        if y >= area.y + area.height { break; }

        let mut col = area.x;
        if row_idx == 0 {
            // First row: render prompt then line content
            for ch in self.prompt.chars() {
                let w = char_width(ch);
                if col + w > area.x + area.width { break; }
                let cell = buf.get_mut(col, y);
                cell.ch = ch; cell.fg = self.prompt_fg; cell.bold = false;
                col += w;
            }
        } else {
            // Continuation rows: indent by prompt_width spaces
            col = area.x + prompt_width;
        }

        for ch in line.chars() {
            let w = char_width(ch);
            if w == 0 { continue; }
            if col + w > area.x + area.width { break; }
            let cell = buf.get_mut(col, y);
            cell.ch = ch; cell.fg = None; cell.bold = false;
            if w == 2 && col + 1 < area.x + area.width {
                let cell2 = buf.get_mut(col + 1, y);
                cell2.ch = '\0'; cell2.fg = None; cell2.bold = false;
            }
            col += w;
        }
    }
}
```

- [ ] **Step 5: Add multiline InputWidget render test to `tests/tui/input_test.rs`**

```rust
#[test]
fn renders_multiline_content() {
    // Content with newline: "ab\ncd"
    let w = InputWidget::new("ab\ncd", 5, "> ");
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 2));
    w.render(Rect::new(0, 0, 20, 2), &mut buf);
    // Row 0: "> ab"
    assert_eq!(buf.get(0, 0).ch, '>');
    assert_eq!(buf.get(2, 0).ch, 'a');
    assert_eq!(buf.get(3, 0).ch, 'b');
    // Row 1: "  cd" (indented by prompt_width=2)
    assert_eq!(buf.get(2, 1).ch, 'c');
    assert_eq!(buf.get(3, 1).ch, 'd');
}

#[test]
fn multiline_cursor_position_second_row() {
    // Content "ab\nc", cursor at byte 4 (after 'c')
    let w = InputWidget::new("ab\nc", 4, "> ");
    let (col, row) = w.cursor_position(Rect::new(0, 0, 20, 2));
    assert_eq!(row, 1);
    assert_eq!(col, 2 + 1); // prompt_width + 1 char 'c'
}
```

- [ ] **Step 6: Run all tests**

```bash
cargo test --test tui_tests input && cargo test --test repl_test
```
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/repl.rs src/tui/input.rs tests/repl_test.rs tests/tui/input_test.rs
git commit -m "feat(repl): multiline LineEditor with Shift+Enter; InputWidget multiline render"
```

---

## Task 7: Permission Widget — TUI-Integrated ask_fn

**Files:**
- Create: `src/tui/permission.rs`
- Create: `tests/tui/permission_test.rs`
- Modify: `src/tui/mod.rs`
- Modify: `tests/tui/mod.rs`
- Modify: `src/repl.rs` (ask_fn rewrite)

- [ ] **Step 1: Write failing tests in `tests/tui/permission_test.rs`**

```rust
use viv::tui::permission::{render_permission_pending, render_permission_result};
use viv::terminal::style::theme;

#[test]
fn pending_line_contains_tool_name() {
    let line = render_permission_pending("Bash", "cmd=\"ls\"");
    let text: String = line.spans.iter().map(|s| s.text.as_str()).collect();
    assert!(text.contains("Bash"), "should contain tool name");
    assert!(text.contains("ls"), "should contain summary");
}

#[test]
fn pending_line_has_suggestion_bullet() {
    let line = render_permission_pending("Read", "path=\"/tmp\"");
    assert_eq!(line.spans[0].fg, Some(theme::SUGGESTION));
}

#[test]
fn result_allowed_is_green() {
    let line = render_permission_result("Bash", "cmd=\"ls\"", true);
    let text: String = line.spans.iter().map(|s| s.text.as_str()).collect();
    assert!(text.contains("Allowed") || text.contains("✓"));
    assert!(line.spans.iter().any(|s| s.fg == Some(theme::SUCCESS)));
}

#[test]
fn result_denied_is_red() {
    let line = render_permission_result("Bash", "cmd=\"rm\"", false);
    let text: String = line.spans.iter().map(|s| s.text.as_str()).collect();
    assert!(text.contains("Denied") || text.contains("✗"));
    assert!(line.spans.iter().any(|s| s.fg == Some(theme::ERROR)));
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --test tui_tests permission 2>&1 | grep -E "error\[|FAILED"
```
Expected: compile error.

- [ ] **Step 3: Create `src/tui/permission.rs`**

```rust
use crate::terminal::style::theme;
use crate::tui::paragraph::{Line, Span};

pub fn render_permission_pending(tool: &str, summary: &str) -> Line {
    Line::from_spans(vec![
        Span::styled("  ◆ Allow ", theme::SUGGESTION, false),
        Span::styled(tool.to_string(), theme::TEXT, false),
        Span::styled(format!("({})? [y/n]", summary), theme::DIM, false),
    ])
}

pub fn render_permission_result(tool: &str, summary: &str, allowed: bool) -> Line {
    if allowed {
        Line::from_spans(vec![
            Span::styled("  ✓ Allowed ", theme::SUCCESS, false),
            Span::styled(tool.to_string(), theme::SUCCESS, false),
            Span::styled(format!("({})", summary), theme::DIM, false),
        ])
    } else {
        Line::from_spans(vec![
            Span::styled("  ✗ Denied  ", theme::ERROR, false),
            Span::styled(tool.to_string(), theme::ERROR, false),
            Span::styled(format!("({})", summary), theme::DIM, false),
        ])
    }
}
```

Add `pub const TEXT: Color = Color::Rgb(255, 255, 255);` is already in `theme` — confirm it exists in `src/terminal/style.rs`. (It does: `TEXT = Color::Rgb(255, 255, 255)`.)

- [ ] **Step 4: Register modules**

`src/tui/mod.rs`:
```rust
pub mod block;
pub mod header;
pub mod input;
pub mod layout;
pub mod message_style;
pub mod paragraph;
pub mod permission;
pub mod renderer;
pub mod spinner;
pub mod status;
pub mod widget;
```

`tests/tui/mod.rs`:
```rust
mod block_test;
mod header_test;
mod input_test;
mod layout_test;
mod message_style_test;
mod paragraph_test;
mod permission_test;
mod renderer_test;
mod spinner_test;
mod status_test;
mod widget_test;
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cargo test --test tui_tests permission
```
Expected: 4 tests pass.

- [ ] **Step 6: Rewrite `ask_fn` in `src/repl.rs` to use TUI rendering**

Replace the existing `ask_fn` closure (lines ~147–174) with:

```rust
// Safety: ask_fn and on_text are never called concurrently —
// ask_fn fires between LLM iterations, on_text fires during streaming.
let history_ptr = &mut history_lines as *mut Vec<Line>;
let renderer_ptr = &mut renderer as *mut Renderer;
let backend_ptr = &mut backend as *mut LinuxBackend;
let scroll_ref = &mut scroll as *mut u16;

let mut ask_fn = move |tool_name: &str, tool_input: &crate::json::JsonValue| -> bool {
    use std::io::Read;
    let history = unsafe { &mut *history_ptr };
    let renderer = unsafe { &mut *renderer_ptr };
    let backend = unsafe { &mut *backend_ptr };
    let scroll = unsafe { &mut *scroll_ref };

    let summary = format_tool_summary(tool_input);
    history.push(crate::tui::permission::render_permission_pending(tool_name, &summary));

    *scroll = compute_max_scroll(history, renderer);
    render_frame(renderer, history, &LineEditor::default(), *scroll);
    let _ = renderer.flush(backend);
    let _ = backend.flush();

    let mut buf = [0u8; 1];
    let allowed = loop {
        match std::io::stdin().lock().read(&mut buf) {
            Ok(1) => break matches!(buf[0], b'y' | b'Y'),
            _ => break false,
        }
    };

    if let Some(last) = history.last_mut() {
        *last = crate::tui::permission::render_permission_result(tool_name, &summary, allowed);
    }
    *scroll = compute_max_scroll(history, renderer);
    render_frame(renderer, history, &LineEditor::default(), *scroll);
    let _ = renderer.flush(backend);
    let _ = backend.flush();

    allowed
};
```

Also update all call sites in `repl.rs` that referenced `editor.buf` and `editor.cursor` to use `editor.content()` and `editor.cursor_offset()`.

In `render_frame`, update the `InputWidget` construction:
```rust
let input_widget = InputWidget::new(&editor.content(), editor.cursor_offset(), "\u{276F} ")
    .prompt_fg(theme::CLAUDE)
    .placeholder(Some("How can I help you?"));
```

- [ ] **Step 7: Run all tests**

```bash
cargo test 2>&1 | grep -E "FAILED|error\[" | head -20
```
Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/tui/permission.rs src/tui/mod.rs tests/tui/permission_test.rs tests/tui/mod.rs src/repl.rs
git commit -m "feat(tui): TUI-integrated permission prompts; replace raw stdout ask_fn"
```

---

## Task 8: Markdown Renderer — MVP Subset

**Files:**
- Create: `src/tui/markdown.rs`
- Create: `tests/tui/markdown_test.rs`
- Modify: `src/tui/mod.rs`
- Modify: `tests/tui/mod.rs`
- Modify: `src/tui/message_style.rs` (`format_assistant_message` uses `render_markdown`)

- [ ] **Step 1: Write failing tests in `tests/tui/markdown_test.rs`**

```rust
use viv::tui::markdown::render_markdown;
use viv::terminal::style::theme;

#[test]
fn plain_text_single_line() {
    let lines = render_markdown("hello world");
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].spans[0].text, "hello world");
}

#[test]
fn bold_double_star() {
    let lines = render_markdown("**bold**");
    assert!(lines[0].spans.iter().any(|s| s.bold && s.text == "bold"));
}

#[test]
fn inline_code_uses_suggestion_color() {
    let lines = render_markdown("`code`");
    assert!(lines[0].spans.iter().any(|s| s.fg == Some(theme::SUGGESTION) && s.text == "code"));
}

#[test]
fn heading_h1_is_bold() {
    let lines = render_markdown("# Title");
    assert!(lines[0].spans.iter().any(|s| s.bold));
    let text: String = lines[0].spans.iter().map(|s| s.text.as_str()).collect();
    assert!(text.contains("Title"));
}

#[test]
fn unordered_list_item() {
    let lines = render_markdown("- item one");
    let text: String = lines[0].spans.iter().map(|s| s.text.as_str()).collect();
    assert!(text.contains("•") || text.contains("item one"));
}

#[test]
fn code_block_indented() {
    let md = "```\nlet x = 1;\n```";
    let lines = render_markdown(md);
    // Should not contain the fence lines; content line should be present
    let all_text: String = lines.iter()
        .flat_map(|l| l.spans.iter())
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join("");
    assert!(all_text.contains("let x = 1;"), "code content should appear");
    assert!(!all_text.contains("```"), "fence markers should not appear");
}

#[test]
fn multiline_plain_text() {
    let lines = render_markdown("line one\nline two");
    assert_eq!(lines.len(), 2);
    assert!(lines[0].spans[0].text.contains("line one"));
    assert!(lines[1].spans[0].text.contains("line two"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --test tui_tests markdown 2>&1 | grep -E "error\[|FAILED"
```
Expected: compile error.

- [ ] **Step 3: Create `src/tui/markdown.rs`**

```rust
use crate::terminal::style::theme;
use crate::tui::paragraph::{Line, Span};

pub fn render_markdown(text: &str) -> Vec<Line> {
    let mut result: Vec<Line> = Vec::new();
    let mut in_code_block = false;

    for raw_line in text.split('\n') {
        if raw_line.starts_with("```") {
            in_code_block = !in_code_block;
            continue; // skip fence lines
        }

        if in_code_block {
            result.push(Line::from_spans(vec![
                Span::raw("  "),
                Span::styled(raw_line.to_string(), theme::TEXT, false),
            ]));
            continue;
        }

        if let Some(rest) = raw_line.strip_prefix("### ") {
            result.push(Line::from_spans(vec![
                Span::raw("   "),
                Span::styled(rest.to_string(), theme::TEXT, true),
            ]));
        } else if let Some(rest) = raw_line.strip_prefix("## ") {
            result.push(Line::from_spans(vec![Span::styled(rest.to_string(), theme::TEXT, true)]));
        } else if let Some(rest) = raw_line.strip_prefix("# ") {
            result.push(Line::from_spans(vec![Span::styled(rest.to_string(), theme::TEXT, true)]));
        } else if raw_line.starts_with("- ") || raw_line.starts_with("* ") {
            let item = &raw_line[2..];
            result.push(Line::from_spans(
                std::iter::once(Span::raw("  • "))
                    .chain(parse_inline(item))
                    .collect(),
            ));
        } else if raw_line.len() > 2
            && raw_line.as_bytes()[0].is_ascii_digit()
            && raw_line.as_bytes()[1] == b'.'
        {
            // Ordered list: "1. item"
            let item = &raw_line[2..].trim_start();
            let num = &raw_line[..raw_line.find('.').unwrap_or(1) + 1];
            result.push(Line::from_spans(
                std::iter::once(Span::raw(format!("  {} ", num)))
                    .chain(parse_inline(item))
                    .collect(),
            ));
        } else {
            result.push(Line::from_spans(parse_inline(raw_line)));
        }
    }

    if result.is_empty() {
        result.push(Line::raw(""));
    }
    result
}

/// Parse inline markdown: **bold**, `code`, plain text. Returns Vec<Span>.
fn parse_inline(text: &str) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    let mut rest = text;

    while !rest.is_empty() {
        if let Some(pos) = rest.find("**") {
            if pos > 0 { spans.push(Span::raw(&rest[..pos])); }
            rest = &rest[pos + 2..];
            if let Some(end) = rest.find("**") {
                spans.push(Span::styled(rest[..end].to_string(), theme::TEXT, true));
                rest = &rest[end + 2..];
            } else {
                spans.push(Span::raw("**"));
            }
        } else if let Some(pos) = rest.find('`') {
            if pos > 0 { spans.push(Span::raw(&rest[..pos])); }
            rest = &rest[pos + 1..];
            if let Some(end) = rest.find('`') {
                spans.push(Span::styled(rest[..end].to_string(), theme::SUGGESTION, false));
                rest = &rest[end + 1..];
            } else {
                spans.push(Span::raw("`"));
            }
        } else {
            spans.push(Span::raw(rest));
            break;
        }
    }

    if spans.is_empty() { spans.push(Span::raw("")); }
    spans
}
```

- [ ] **Step 4: Wire `render_markdown` into `format_assistant_message`**

In `src/tui/message_style.rs`, replace `format_assistant_message`:
```rust
pub fn format_assistant_message(response: &str) -> Vec<Line> {
    use crate::tui::markdown::render_markdown;
    let md_lines = render_markdown(response);
    let mut result = Vec::new();
    for (i, line) in md_lines.into_iter().enumerate() {
        if i == 0 {
            let mut spans = vec![Span::styled("● ", theme::CLAUDE, false)];
            spans.extend(line.spans);
            result.push(Line::from_spans(spans));
        } else {
            let mut spans = vec![Span::raw("  ")];
            spans.extend(line.spans);
            result.push(Line::from_spans(spans));
        }
    }
    result
}
```

- [ ] **Step 5: Register modules**

`src/tui/mod.rs`: add `pub mod markdown;`  
`tests/tui/mod.rs`: add `mod markdown_test;`

- [ ] **Step 6: Run all tui tests**

```bash
cargo test --test tui_tests
```
Expected: all pass.

- [ ] **Step 7: Verify `format_assistant_message` existing tests still pass**

```bash
cargo test --test tui_tests message_style
```
Expected: all pass (the multi-line and empty-input tests will now go through `render_markdown` but produce the same output for plain text).

- [ ] **Step 8: Commit**

```bash
git add src/tui/markdown.rs src/tui/message_style.rs src/tui/mod.rs \
        tests/tui/markdown_test.rs tests/tui/mod.rs
git commit -m "feat(tui): markdown renderer (bold, inline-code, code blocks, lists, headings)"
```

---

## Task 9: Integration — Layout, Welcome Message, Wire Everything into repl.rs

**Files:**
- Modify: `src/repl.rs` (layout, render_frame, welcome, status bar, header)
- Modify: `src/tui/message_style.rs` (`format_welcome` signature)
- Modify: `tests/tui/message_style_test.rs` (update welcome test)

- [ ] **Step 1: Update `format_welcome` signature in `src/tui/message_style.rs`**

Replace:
```rust
pub fn format_welcome() -> Line {
    Line::from_spans(vec![
        Span::styled("● ", theme::CLAUDE, false),
        Span::styled("viv", theme::CLAUDE, true),
        Span::raw("  "),
        Span::styled("ready", theme::DIM, false),
    ])
}
```

With:
```rust
pub fn format_welcome(cwd: &str, branch: Option<&str>) -> Line {
    let mut spans = vec![
        Span::styled("● ", theme::CLAUDE, false),
        Span::styled("viv", theme::CLAUDE, true),
        Span::raw("  "),
        Span::styled(cwd.to_string(), theme::DIM, false),
    ];
    if let Some(b) = branch {
        spans.push(Span::styled("  ⎇ ".to_string(), theme::DIM, false));
        spans.push(Span::styled(b.to_string(), theme::DIM, false));
    }
    Line::from_spans(spans)
}
```

- [ ] **Step 2: Update welcome test in `tests/tui/message_style_test.rs`**

Replace the `welcome_line_is_single_bullet_plus_ready` test:
```rust
#[test]
fn welcome_line_has_cwd_and_optional_branch() {
    let line = format_welcome("~/project", Some("main"));
    assert_eq!(line.spans[0].text, "● ");
    assert_eq!(line.spans[0].fg, Some(theme::CLAUDE));
    assert_eq!(line.spans[1].text, "viv");
    assert!(line.spans[1].bold);
    // cwd appears
    let all_text: String = line.spans.iter().map(|s| s.text.as_str()).collect();
    assert!(all_text.contains("~/project"));
    assert!(all_text.contains("main"));
}

#[test]
fn welcome_line_without_branch() {
    let line = format_welcome("~/project", None);
    let all_text: String = line.spans.iter().map(|s| s.text.as_str()).collect();
    assert!(!all_text.contains('⎇'));
}
```

- [ ] **Step 3: Run message_style tests to confirm they pass**

```bash
cargo test --test tui_tests message_style
```
Expected: all pass.

- [ ] **Step 4: Rewrite `main_layout` to accept `input_height` and add 4 zones**

In `src/repl.rs`, replace `main_layout()`:
```rust
fn main_layout(input_height: u16) -> Layout {
    Layout::new(Direction::Vertical).constraints(vec![
        Constraint::Fixed(1),          // header
        Constraint::Fill,              // conversation
        Constraint::Fixed(input_height), // input box (dynamic)
        Constraint::Fixed(1),          // status bar
    ])
}
```

- [ ] **Step 5: Update `render_frame` signature and body**

Replace `render_frame`:
```rust
fn render_frame(
    renderer: &mut Renderer,
    history_lines: &[Line],
    editor: &LineEditor,
    scroll: u16,
    header: &crate::tui::header::HeaderWidget,
    status: &crate::tui::status::StatusWidget,
) {
    let area = renderer.area();
    let input_height = (editor.line_count() as u16 + 2).min(8);
    let chunks = main_layout(input_height).split(area);
    let buf = renderer.buffer_mut();

    // Header
    header.render(chunks[0], buf);

    // Conversation
    let paragraph = Paragraph::new(history_lines.to_vec()).scroll(scroll);
    paragraph.render(chunks[1], buf);

    // Input box
    let input_block = Block::new()
        .border(BorderStyle::Rounded)
        .borders(BorderSides::HORIZONTAL)
        .border_fg(theme::DIM);
    let input_inner = input_block.inner(chunks[2]);
    input_block.render(chunks[2], buf);
    let input_widget = InputWidget::new(&editor.content(), editor.cursor_offset(), "\u{276F} ")
        .prompt_fg(theme::CLAUDE)
        .placeholder(Some("How can I help you?"));
    input_widget.render(input_inner, buf);

    // Status bar
    status.render(chunks[3], buf);
}
```

- [ ] **Step 6: Update `compute_max_scroll` and cursor logic**

Replace `compute_max_scroll`:
```rust
fn compute_max_scroll(history_lines: &[Line], renderer: &Renderer, editor: &LineEditor) -> u16 {
    let area = renderer.area();
    let input_height = (editor.line_count() as u16 + 2).min(8);
    let chunks = main_layout(input_height).split(area);
    let conv_height = chunks[1].height as usize;
    let conv_width = chunks[1].width as usize;
    if conv_width == 0 || conv_height == 0 { return 0; }
    let total_rows: usize = history_lines.iter().map(|l| count_wrapped_rows(l, conv_width)).sum();
    if total_rows > conv_height { (total_rows - conv_height) as u16 } else { 0 }
}
```

Update all call sites in `run()` to pass `&editor`.

- [ ] **Step 7: Update cursor positioning in the render loop**

Find the cursor positioning block in `run()` (after `renderer.flush`):
```rust
let input_height = (editor.line_count() as u16 + 2).min(8);
let chunks = main_layout(input_height).split(area);
let input_block = Block::new()
    .border(BorderStyle::Rounded)
    .borders(BorderSides::HORIZONTAL)
    .border_fg(theme::DIM);
let input_inner = input_block.inner(chunks[2]);
let input_widget = InputWidget::new(&editor.content(), editor.cursor_offset(), "\u{276F} ")
    .prompt_fg(theme::CLAUDE)
    .placeholder(Some("How can I help you?"));
let (cx, cy) = input_widget.cursor_position(input_inner);
```

- [ ] **Step 8: Construct HeaderWidget and StatusWidget in `run()` and thread through**

In `run()`, after creating `agent_ctx`:
```rust
let header = crate::tui::header::HeaderWidget::from_env();
```

Build status widget before each `render_frame` call:
```rust
let status = crate::tui::status::StatusWidget {
    model: agent_ctx.llm.config.model(crate::llm::ModelTier::Medium).to_string(),
    input_tokens: agent_ctx.input_tokens,
    output_tokens: agent_ctx.output_tokens,
};
```

Update welcome call:
```rust
history_lines.push(format_welcome(&header.cwd, header.branch.as_deref()));
```

Update all `render_frame(...)` calls to include `&header, &status`.

- [ ] **Step 9: Build and verify compilation**

```bash
cargo build 2>&1 | grep -E "error\[" | head -30
```
Expected: zero errors.

- [ ] **Step 10: Run full test suite**

```bash
cargo test 2>&1 | grep -E "FAILED|error\[" | head -20
```
Expected: zero failures.

- [ ] **Step 11: Commit**

```bash
git add src/repl.rs src/tui/message_style.rs tests/tui/message_style_test.rs
git commit -m "feat(repl): 4-zone layout with header, status bar, multiline input, and welcome with cwd"
```

---

## Final Verification

- [ ] **Build release and verify no warnings**

```bash
cargo build --release 2>&1 | grep -E "warning:|error\["
```

- [ ] **Run full test suite**

```bash
cargo test 2>&1 | tail -5
```
Expected: `test result: ok. N passed; 0 failed`.

- [ ] **Smoke-test: start viv and verify layout visually**

```bash
VIV_API_KEY=xxx cargo run
```
Check:
- Header row shows cwd and git branch
- Welcome message shows cwd/branch
- Input box shows `How can I help you?` placeholder
- Footer shows model name and `↑ 0  ↓ 0  ~$0.000`
- User messages: `>` in orange
- After a response: `●` prefix, markdown rendered (send `**hello**` and verify bold)
- Shift+Enter inserts a newline in the input box
