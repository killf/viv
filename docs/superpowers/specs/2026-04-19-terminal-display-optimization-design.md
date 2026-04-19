# Terminal Display Optimization Design

Date: 2026-04-19

## Problem

The TUI layer (built 2026-04-18) has two categories of issues:

1. **Text truncation** — `MarkdownBlockWidget` renders Paragraph, Heading, Quote, and List items as single rows. Content wider than the terminal is silently clipped. `UserMessage` is also hardcoded to height 1. The word-wrap algorithm in `paragraph.rs` (`wrap_line`) exists but is not used by the Markdown renderer.

2. **Limited visual styling** — `Cell` only has `bold: bool`; italic text falls back to dim color. Code blocks lack background color. Inline code shares the brand color. No spacing between blocks. All heading levels look identical.

## Changes

### 1. Cell Style Extension

**File:** `src/core/terminal/buffer.rs`

Add two fields to `Cell`:

```rust
pub struct Cell {
    pub ch: char,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,   // NEW
    pub dim: bool,       // NEW
}
```

**File:** `src/core/terminal/buffer.rs` (`Buffer::diff`)

After bold, emit italic (`\x1b[3m`) and dim (`\x1b[2m`) when set. The existing `reset_style` (`\x1b[0m`) already clears all attributes.

**File:** `src/tui/paragraph.rs`

`Span` gains `italic: bool` and `dim: bool`. `StyledChar` likewise. `wrap_line` and `Paragraph::render` propagate these to cells.

### 2. Markdown Word-Wrap

**File:** `src/tui/paragraph.rs`

- Make `wrap_line` public: `pub fn wrap_line(...)`.

**File:** `src/tui/markdown.rs`

Replace `render_inline_spans` (character-by-character, no wrap) with a flow that:

1. Converts `Vec<InlineSpan>` to a `Line` (via `inline_span_to_span`).
2. Calls `wrap_line(&line, width)` to get physical rows.
3. Renders each physical row with proper styling.

Update `node_height`:

| Node | Height calculation |
|------|-------------------|
| Paragraph | `wrap_line(spans_to_line(spans), width).len()` |
| Heading | `wrap_line(spans_to_line(text), width).len()` |
| Quote | `wrap_line(spans_to_line(spans), width - 2).len()` (2 = `"│ "` prefix) |
| List item | `wrap_line(spans_to_line(item), width - 4).len()` per item, summed (4 = `"  • "` prefix) |
| CodeBlock | unchanged (delegates to `CodeBlockWidget::height`) |
| HorizontalRule | 1 |

**File:** `src/bus/terminal.rs`

`block_height_with_width` for `UserMessage`: compute via `wrap_line` with `width - 2` (for `"> "` prefix). Render wrapped rows accordingly in `render_block`.

### 3. Block Spacing

**File:** `src/tui/markdown.rs`

`MarkdownBlockWidget::height` adds 1 spacing row between adjacent nodes. Specifically, for N nodes, add (N-1) spacing rows. `HorizontalRule` is not special-cased — it gets spacing like everything else.

The render loop increments `row += 1` after each node (except the last) to insert the blank line.

This gives visual breathing room between paragraphs, headings, lists, and code blocks.

### 4. Code Block Background

**File:** `src/tui/code_block.rs`

Before rendering tokens, fill the inner area cells with `bg = Some(Color::Rgb(30, 30, 30))`. This makes code blocks visually distinct from surrounding prose.

### 5. Inline Code Style

**File:** `src/tui/markdown.rs` (`inline_span_to_span`)

Change `InlineSpan::Code` from:
- fg `Rgb(215, 119, 87)` (same as brand color)

To:
- fg `Rgb(230, 150, 100)` (warm orange, distinct from brand)
- bg `Rgb(45, 40, 38)` (subtle dark background)

This requires `Span` to carry an optional `bg` (added in change 1).

### 6. Heading Level Distinction

**File:** `src/tui/markdown.rs`

| Level | Style |
|-------|-------|
| h1 | bold + `theme::CLAUDE` (brand orange) |
| h2 | bold + `theme::TEXT` (white) — current behavior |
| h3-h6 | bold + `theme::DIM` (gray) |

### 7. Quote Italic + Preserve Colors

**File:** `src/tui/markdown.rs`

Quote spans retain their original fg color from `inline_span_to_span` but add `italic: true`. The quote prefix `"│ "` stays `Rgb(100, 100, 100)`.

### 8. Italic Span Rendering

**File:** `src/tui/markdown.rs` (`inline_span_to_span`)

Change `InlineSpan::Italic` from:
- fg `theme::DIM`, bold false

To:
- fg `theme::TEXT` (white), bold false, italic true

### 9. Neofetch-Style Welcome Screen

Replace the current single-line welcome (`● viv  <cwd>  ⎇ <branch>  ready`) with a neofetch-style startup screen: ASCII art logo on the left, system info on the right.

**Layout:**

```
       _            Model:     claude-sonnet-4-6
__   _(_)_   __     CWD:       ~/data/dlab/viv
\ \ / / \ \ / /    Branch:    main
 \ V /| |\ V /     Platform:  Linux x86_64
  \_/ |_| \_/      Shell:     zsh
```

**Style:**
- Logo: CLAUDE orange `Rgb(215, 119, 87)`
- Info labels (Model, CWD, ...): CLAUDE orange, bold
- Info values: TEXT white
- Gap between logo and info: 4 columns

**Info items:**

| Item | Source |
|------|--------|
| Model | `self.model_name` from `AgentMessage::Ready` |
| CWD | `std::env::current_dir()`, `~`-collapsed |
| Branch | `.git/HEAD` parse |
| Platform | compile-time `cfg!(target_os)` + `cfg!(target_arch)` |
| Shell | `$SHELL` env var, basename only |

**Implementation:**

- Add `ContentBlock::Welcome` variant with fields: `model: Option<String>`, `cwd: String`, `branch: Option<String>`.
- New `WelcomeWidget` in `src/tui/welcome.rs`. The logo is a `const &str` array (5 lines). Info items are rendered line-by-line to the right of the logo.
- Fixed height: 5 rows (matching the logo height).
- On startup, push `ContentBlock::Welcome` with `model: None`. When `AgentMessage::Ready { model }` arrives, update the existing Welcome block's model field and mark dirty. Before the model arrives, display `"..."` as placeholder.

**Files:**
- New: `src/tui/welcome.rs`
- Modified: `src/tui/content.rs` (add `ContentBlock::Welcome`)
- Modified: `src/bus/terminal.rs` (replace welcome init + handle Ready update + `render_block` + `block_height_with_width`)
- Modified: `src/tui/mod.rs` (add `pub mod welcome`)

## Files Modified

| File | Changes |
|------|---------|
| `src/core/terminal/buffer.rs` | Cell: +italic, +dim. Buffer::diff: emit italic/dim ANSI. |
| `src/tui/paragraph.rs` | Span: +italic, +dim, +bg. StyledChar: same. `wrap_line` made pub. Propagate new fields in render. |
| `src/tui/markdown.rs` | Rewrite render to use wrap_line. Update node_height. Block spacing. Heading levels. Quote italic. Inline code style. Italic span. |
| `src/tui/code_block.rs` | Fill inner bg before token rendering. |
| `src/tui/welcome.rs` | NEW: WelcomeWidget with ASCII art logo + system info. |
| `src/tui/content.rs` | Add `ContentBlock::Welcome` variant. |
| `src/tui/mod.rs` | Add `pub mod welcome`. |
| `src/bus/terminal.rs` | Welcome screen init + Ready update. UserMessage wrap + height. render_block for Welcome/UserMessage. |

## Not In Scope

- Underline attribute (no Markdown syntax for it, YAGNI)
- Mouse support
- 256-color palette indexing
- Removing legacy `screen.rs` (separate cleanup)
- Code block line-wrap (code is conventionally not wrapped; horizontal scroll would be the proper future solution)
