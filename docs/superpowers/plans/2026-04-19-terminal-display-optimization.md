# Terminal Display Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix text truncation (word-wrap all Markdown/UserMessage content) and upgrade visual styling (italic/dim, code block backgrounds, heading levels, block spacing, neofetch welcome screen).

**Architecture:** Bottom-up changes — extend `Cell` and `Span` with new style fields first, then rewrite `MarkdownBlockWidget` to use `wrap_line`, then add visual enhancements, then the welcome screen. Existing tests updated in each task to match new behavior.

**Tech Stack:** Rust (edition 2024), zero dependencies, ANSI escape sequences, custom TUI framework.

---

## File Structure

| File | Role | Action |
|------|------|--------|
| `src/core/terminal/buffer.rs` | Cell struct + Buffer::diff | Modify: add italic/dim to Cell |
| `src/tui/paragraph.rs` | Span/Line/wrap_line/Paragraph | Modify: add italic/dim/bg to Span, make wrap_line pub |
| `src/tui/markdown.rs` | MarkdownBlockWidget rendering | Modify: rewrite to use wrap_line, add spacing, visual styles |
| `src/tui/code_block.rs` | CodeBlockWidget | Modify: fill inner bg |
| `src/tui/content.rs` | ContentBlock enum | Modify: add Welcome variant |
| `src/tui/welcome.rs` | WelcomeWidget (neofetch) | Create |
| `src/tui/mod.rs` | Module exports | Modify: add `pub mod welcome` |
| `src/bus/terminal.rs` | TerminalUI orchestration | Modify: UserMessage wrap + Welcome screen wiring |
| `tests/tui/paragraph_test.rs` | Paragraph tests | Modify: test new Span fields |
| `tests/tui/markdown_test.rs` | Markdown widget tests | Modify: test wrap, spacing, new colors |
| `tests/tui/code_block_test.rs` | CodeBlock tests | Modify: test bg fill |
| `tests/tui/welcome_test.rs` | Welcome widget tests | Create |
| `tests/tui/mod.rs` | Test module index | Modify: add welcome_test |
| `tests/core/terminal/screen_test.rs` | Buffer/Cell tests | Modify: test italic/dim in diff |

---

### Task 1: Cell Style Extension

**Files:**
- Modify: `src/core/terminal/buffer.rs`
- Modify: `tests/core/terminal/screen_test.rs`

- [ ] **Step 1: Write failing tests for Cell italic/dim fields**

In `tests/core/terminal/screen_test.rs`, add:

```rust
#[test]
fn cell_italic_default_false() {
    let cell = Cell::default();
    assert!(!cell.italic);
    assert!(!cell.dim);
}

#[test]
fn diff_emits_italic_ansi() {
    let area = Rect::new(0, 0, 3, 1);
    let prev = Buffer::empty(area);
    let mut curr = Buffer::empty(area);
    let cell = curr.get_mut(0, 0);
    cell.ch = 'a';
    cell.italic = true;
    let bytes = curr.diff(&prev);
    let output = String::from_utf8_lossy(&bytes);
    // \x1b[3m is ANSI italic
    assert!(output.contains("\x1b[3m"), "diff should emit italic ANSI: {output:?}");
}

#[test]
fn diff_emits_dim_ansi() {
    let area = Rect::new(0, 0, 3, 1);
    let prev = Buffer::empty(area);
    let mut curr = Buffer::empty(area);
    let cell = curr.get_mut(0, 0);
    cell.ch = 'a';
    cell.dim = true;
    let bytes = curr.diff(&prev);
    let output = String::from_utf8_lossy(&bytes);
    // \x1b[2m is ANSI dim
    assert!(output.contains("\x1b[2m"), "diff should emit dim ANSI: {output:?}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test core_tests -- screen_test::cell_italic_default_false screen_test::diff_emits_italic_ansi screen_test::diff_emits_dim_ansi`
Expected: compilation error — `Cell` has no field `italic`/`dim`.

- [ ] **Step 3: Add italic/dim fields to Cell**

In `src/core/terminal/buffer.rs`, update `Cell`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub dim: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Cell {
            ch: ' ',
            fg: None,
            bg: None,
            bold: false,
            italic: false,
            dim: false,
        }
    }
}
```

- [ ] **Step 4: Update Buffer::diff to emit italic/dim ANSI**

In `src/core/terminal/buffer.rs`, in the `diff` method, after the bold block:

```rust
            if cell.bold {
                writer.bold();
            }

            if cell.italic {
                writer.write_bytes(b"\x1b[3m");
            }

            if cell.dim {
                writer.write_bytes(b"\x1b[2m");
            }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test core_tests -- screen_test`
Expected: all screen_test tests PASS.

- [ ] **Step 6: Commit**

```bash
git add src/core/terminal/buffer.rs tests/core/terminal/screen_test.rs
git commit -m "feat(tui): add italic and dim fields to Cell"
```

---

### Task 2: Span/StyledChar Extension + Public wrap_line

**Files:**
- Modify: `src/tui/paragraph.rs`
- Modify: `tests/tui/paragraph_test.rs`

- [ ] **Step 1: Write failing tests for Span with italic/dim/bg**

In `tests/tui/paragraph_test.rs`, add:

```rust
#[test]
fn span_italic_renders_to_cell() {
    let line = Line::from_spans(vec![Span {
        text: "hi".to_string(),
        fg: None,
        bg: None,
        bold: false,
        italic: true,
        dim: false,
    }]);
    let p = Paragraph::new(vec![line]);
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
    p.render(Rect::new(0, 0, 10, 1), &mut buf);
    assert!(buf.get(0, 0).italic, "italic span should set cell.italic");
}

#[test]
fn span_bg_renders_to_cell() {
    use viv::core::terminal::style::Color;
    let line = Line::from_spans(vec![Span {
        text: "x".to_string(),
        fg: None,
        bg: Some(Color::Rgb(45, 40, 38)),
        bold: false,
        italic: false,
        dim: false,
    }]);
    let p = Paragraph::new(vec![line]);
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
    p.render(Rect::new(0, 0, 10, 1), &mut buf);
    assert_eq!(buf.get(0, 0).bg, Some(Color::Rgb(45, 40, 38)));
}

#[test]
fn wrap_line_is_accessible() {
    use viv::tui::paragraph::wrap_line;
    let line = Line::raw("hello world foo bar");
    let rows = wrap_line(&line, 10);
    assert!(rows.len() >= 2, "should wrap into multiple rows");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test tui_tests -- paragraph_test::span_italic_renders_to_cell paragraph_test::span_bg_renders_to_cell paragraph_test::wrap_line_is_accessible`
Expected: compilation error — `Span` has no field `italic`/`bg`/`dim`, `wrap_line` not public.

- [ ] **Step 3: Extend Span with italic/dim/bg fields**

In `src/tui/paragraph.rs`, update `Span`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub text: String,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub dim: bool,
}

impl Span {
    pub fn raw(text: impl Into<String>) -> Self {
        Span {
            text: text.into(),
            fg: None,
            bg: None,
            bold: false,
            italic: false,
            dim: false,
        }
    }

    pub fn styled(text: impl Into<String>, fg: Color, bold: bool) -> Self {
        Span {
            text: text.into(),
            fg: Some(fg),
            bg: None,
            bold,
            italic: false,
            dim: false,
        }
    }
}
```

- [ ] **Step 4: Extend StyledChar and propagate in render**

In `src/tui/paragraph.rs`, update `StyledChar`:

```rust
struct StyledChar {
    ch: char,
    fg: Option<Color>,
    bg: Option<Color>,
    bold: bool,
    italic: bool,
    dim: bool,
    width: u16,
}
```

Update `wrap_line` to propagate `bg`, `italic`, `dim` when creating `StyledChar` from `Span`.

Update `Paragraph::render` to set `cell.bg`, `cell.italic`, `cell.dim` from `StyledChar`.

- [ ] **Step 5: Make wrap_line and StyledChar public**

Change `fn wrap_line(` to `pub fn wrap_line(`.

Make `StyledChar` and its fields public so `MarkdownBlockWidget` can access the wrapped row data:

```rust
pub struct StyledChar {
    pub ch: char,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub dim: bool,
    pub width: u16,
}
```

- [ ] **Step 6: Fix existing paragraph tests that construct Span**

The test `styled_spans` in `tests/tui/paragraph_test.rs` uses `Span::styled(...)` which still works. No changes needed for existing tests — `Span::raw` and `Span::styled` constructors still provide the same API.

- [ ] **Step 7: Run all paragraph tests**

Run: `cargo test --test tui_tests -- paragraph_test`
Expected: all PASS.

- [ ] **Step 8: Commit**

```bash
git add src/tui/paragraph.rs tests/tui/paragraph_test.rs
git commit -m "feat(tui): extend Span with italic/dim/bg, make wrap_line pub"
```

---

### Task 3: Markdown Word-Wrap + Block Spacing

**Files:**
- Modify: `src/tui/markdown.rs`
- Modify: `tests/tui/markdown_test.rs`

- [ ] **Step 1: Write failing test for paragraph word-wrap**

In `tests/tui/markdown_test.rs`, add:

```rust
#[test]
fn paragraph_wraps_long_text() {
    // "hello world" in a 6-wide area should wrap to 2 rows
    let nodes = parse_markdown("hello world");
    let h = MarkdownBlockWidget::height(&nodes, 6);
    assert_eq!(h, 2, "long paragraph should wrap to 2 rows in width 6");
}

#[test]
fn paragraph_renders_wrapped_second_row() {
    let nodes = parse_markdown("hello world");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 6, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // "hello " on row 0, "world" on row 1
    assert_eq!(buf.get(0, 0).ch, 'h');
    assert_eq!(buf.get(0, 1).ch, 'w', "wrapped word should appear on row 1");
}
```

- [ ] **Step 2: Write failing test for block spacing**

```rust
#[test]
fn block_spacing_between_nodes() {
    // Two paragraphs should have 1 blank row between them
    // "Hello\n\nWorld" => Paragraph("Hello") + Paragraph("World")
    // Height = 1 + 1 (spacing) + 1 = 3
    let nodes = parse_markdown("Hello\n\nWorld");
    let h = MarkdownBlockWidget::height(&nodes, 80);
    assert_eq!(h, 3, "two paragraphs should have 1 row spacing: 1+1+1=3");
}
```

- [ ] **Step 3: Write failing test for list item wrap**

```rust
#[test]
fn list_item_wraps_long_text() {
    // List prefix "  • " is 4 chars, so effective width = 10 - 4 = 6
    // "hello world" needs 2 rows at width 6
    let nodes = parse_markdown("- hello world");
    let h = MarkdownBlockWidget::height(&nodes, 10);
    assert_eq!(h, 2, "long list item should wrap");
}
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test --test tui_tests -- markdown_test::paragraph_wraps markdown_test::block_spacing markdown_test::list_item_wraps`
Expected: FAIL — current heights return 1 for everything.

- [ ] **Step 5: Rewrite node_height to use wrap_line**

In `src/tui/markdown.rs`, add a helper to convert `Vec<InlineSpan>` to a `Line`:

```rust
use crate::tui::paragraph::{Line, Span, wrap_line};

fn spans_to_line(spans: &[InlineSpan], bold_context: bool) -> Line {
    let line_spans: Vec<Span> = spans.iter().map(|s| inline_span_to_span(s, bold_context)).collect();
    Line::from_spans(line_spans)
}
```

Rewrite `node_height`:

```rust
fn node_height(node: &MarkdownNode, width: u16) -> u16 {
    let w = width as usize;
    match node {
        MarkdownNode::Heading { text, .. } => {
            wrap_line(&spans_to_line(text, true), w).len() as u16
        }
        MarkdownNode::Paragraph { spans } => {
            wrap_line(&spans_to_line(spans, false), w).len() as u16
        }
        MarkdownNode::List { ordered, items } => {
            let prefix_width = if *ordered { 5 } else { 4 }; // "  1. " or "  • "
            let inner_w = w.saturating_sub(prefix_width);
            items.iter().map(|item| {
                wrap_line(&spans_to_line(item, false), inner_w.max(1)).len() as u16
            }).sum()
        }
        MarkdownNode::Quote { spans } => {
            let inner_w = w.saturating_sub(2); // "│ " prefix
            wrap_line(&spans_to_line(spans, false), inner_w.max(1)).len() as u16
        }
        MarkdownNode::CodeBlock { code, .. } => CodeBlockWidget::height(code, width),
        MarkdownNode::HorizontalRule => 1,
    }
}
```

Update `MarkdownBlockWidget::height` to add spacing:

```rust
pub fn height(nodes: &[MarkdownNode], width: u16) -> u16 {
    let mut total: u16 = 0;
    for (i, node) in nodes.iter().enumerate() {
        total = total.saturating_add(node_height(node, width));
        // Add 1 row spacing between nodes (not after the last)
        if i + 1 < nodes.len() {
            total = total.saturating_add(1);
        }
    }
    total
}
```

- [ ] **Step 6: Rewrite MarkdownBlockWidget::render to use wrap_line**

Replace `render_inline_spans` with a new function `render_wrapped_spans` that:
1. Converts spans to a `Line`
2. Calls `wrap_line(&line, width)` to get physical rows
3. Renders each `StyledChar` row into the buffer

```rust
fn render_wrapped_spans(
    spans: &[InlineSpan],
    start_x: u16,
    start_y: u16,
    buf: &mut Buffer,
    area: Rect,
    bold_context: bool,
    available_width: u16,
) -> u16 {
    let line = spans_to_line(spans, bold_context);
    let rows = wrap_line(&line, available_width as usize);
    let mut rows_rendered: u16 = 0;
    for (row_idx, row) in rows.iter().enumerate() {
        let y = start_y + row_idx as u16;
        if y >= area.y + area.height {
            break;
        }
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
            cell.fg = sc.fg;
            cell.bg = sc.bg;
            cell.bold = sc.bold;
            cell.italic = sc.italic;
            cell.dim = sc.dim;
            if sc.width == 2 && x + 1 < area.x + area.width {
                let cell2 = buf.get_mut(x + 1, y);
                cell2.ch = '\0';
                cell2.fg = sc.fg;
                cell2.bg = sc.bg;
                cell2.bold = sc.bold;
                cell2.italic = sc.italic;
                cell2.dim = sc.dim;
            }
            x += sc.width;
        }
        rows_rendered += 1;
    }
    rows_rendered
}
```

Note: `wrap_line` returns `Vec<Vec<StyledChar>>` but `StyledChar` is private. We need to make `StyledChar` public (or return a more accessible type). The simplest approach: make `StyledChar` public in `paragraph.rs` by adding `pub` to the struct and its fields.

Update the render match arms to use `render_wrapped_spans` instead of `render_inline_spans`, with spacing between nodes (`row += 1` after each node except the last).

- [ ] **Step 7: Update the height_calculation test**

The existing `height_calculation` test in `tests/tui/markdown_test.rs` expects height=5 for `"# Title\nSome text\n- one\n- two\n---"`. With spacing between 4 nodes (heading, paragraph, list, hr), it becomes:
- heading(1) + spacing(1) + paragraph(1) + spacing(1) + list(2) + spacing(1) + hr(1) = 8

Update:

```rust
#[test]
fn height_calculation() {
    // heading(1) + spacing(1) + paragraph(1) + spacing(1) + list(2 items, 2) + spacing(1) + hr(1) = 8
    let nodes = parse_markdown("# Title\nSome text\n- one\n- two\n---");
    let h = MarkdownBlockWidget::height(&nodes, 80);
    assert_eq!(h, 8, "height should include spacing between nodes");
}
```

- [ ] **Step 8: Run all markdown tests**

Run: `cargo test --test tui_tests -- markdown_test`
Expected: all PASS.

- [ ] **Step 9: Commit**

```bash
git add src/tui/paragraph.rs src/tui/markdown.rs tests/tui/markdown_test.rs
git commit -m "feat(tui): word-wrap for all Markdown content + block spacing"
```

---

### Task 4: Visual Styling — Heading Levels, Inline Code, Italic, Quotes

**Files:**
- Modify: `src/tui/markdown.rs`
- Modify: `tests/tui/markdown_test.rs`

- [ ] **Step 1: Write failing tests for new visual styles**

In `tests/tui/markdown_test.rs`, update and add:

```rust
#[test]
fn heading_h1_uses_claude_color() {
    use viv::core::terminal::style::Color;
    let nodes = parse_markdown("# Title");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let cell = buf.get(0, 0);
    assert_eq!(cell.fg, Some(Color::Rgb(215, 119, 87)), "h1 should use CLAUDE orange");
    assert!(cell.bold, "h1 should be bold");
}

#[test]
fn heading_h3_uses_dim_color() {
    use viv::core::terminal::style::Color;
    let nodes = parse_markdown("### Subtitle");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let cell = buf.get(0, 0);
    assert_eq!(cell.fg, Some(Color::Rgb(136, 136, 136)), "h3 should use DIM gray");
    assert!(cell.bold, "h3 should be bold");
}

#[test]
fn inline_code_uses_new_color() {
    use viv::core::terminal::style::Color;
    let nodes = parse_markdown("use `cargo`");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // Find 'c' of "cargo"
    let code_cell = (0..area.width).map(|x| buf.get(x, 0)).find(|c| c.ch == 'c');
    let cell = code_cell.expect("should find 'c'");
    assert_eq!(cell.fg, Some(Color::Rgb(230, 150, 100)), "inline code should use new orange");
    assert_eq!(cell.bg, Some(Color::Rgb(45, 40, 38)), "inline code should have subtle bg");
}

#[test]
fn italic_uses_italic_flag() {
    let nodes = parse_markdown("*hello*");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let cell = buf.get(0, 0);
    assert!(cell.italic, "italic text should set cell.italic");
    assert_eq!(cell.fg, Some(viv::core::terminal::style::Color::Rgb(255, 255, 255)),
        "italic should use TEXT white, not DIM");
}

#[test]
fn quote_preserves_bold_and_adds_italic() {
    let nodes = parse_markdown("> **bold** text");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // After "│ " (2 chars), the bold text starts at x=2
    let bold_cell = buf.get(2, 0);
    assert!(bold_cell.bold, "bold inside quote should stay bold");
    assert!(bold_cell.italic, "quote content should be italic");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test tui_tests -- markdown_test::heading_h1_uses_claude markdown_test::heading_h3_uses_dim markdown_test::inline_code_uses_new markdown_test::italic_uses_italic markdown_test::quote_preserves`
Expected: FAIL — old colors/no italic flag.

- [ ] **Step 3: Update inline_span_to_span for new styles**

In `src/tui/markdown.rs`:

```rust
fn inline_span_to_span(span: &InlineSpan, bold_context: bool) -> Span {
    match span {
        InlineSpan::Text(s) => Span {
            text: s.clone(),
            fg: Some(theme::TEXT),
            bg: None,
            bold: bold_context,
            italic: false,
            dim: false,
        },
        InlineSpan::Bold(s) => Span {
            text: s.clone(),
            fg: Some(theme::TEXT),
            bg: None,
            bold: true,
            italic: false,
            dim: false,
        },
        InlineSpan::Italic(s) => Span {
            text: s.clone(),
            fg: Some(theme::TEXT),
            bg: None,
            bold: false,
            italic: true,
            dim: false,
        },
        InlineSpan::Code(s) => Span {
            text: s.clone(),
            fg: Some(Color::Rgb(230, 150, 100)),
            bg: Some(Color::Rgb(45, 40, 38)),
            bold: false,
            italic: false,
            dim: false,
        },
        InlineSpan::Link { text, .. } => Span {
            text: text.clone(),
            fg: Some(Color::Rgb(100, 150, 255)),
            bg: None,
            bold: false,
            italic: false,
            dim: false,
        },
    }
}
```

- [ ] **Step 4: Update heading rendering for level distinction**

In the render match arm for `MarkdownNode::Heading`, determine the fg color by level:

```rust
MarkdownNode::Heading { level, text } => {
    let heading_fg = match level {
        1 => theme::CLAUDE,
        2 => theme::TEXT,
        _ => theme::DIM,
    };
    // Override the spans' fg to heading_fg while keeping bold
    let line = spans_to_line_with_fg(text, heading_fg);
    // ... render wrapped
}
```

Add a helper `spans_to_line_with_fg` that creates a Line where all spans use the given fg and bold=true.

- [ ] **Step 5: Update quote rendering to add italic and preserve colors**

In the render match arm for `MarkdownNode::Quote`, after converting spans, set `italic = true` on all spans while preserving their original fg:

```rust
MarkdownNode::Quote { spans } => {
    // Render "│ " prefix
    // ...
    // Convert spans with italic added
    let mut line = spans_to_line(spans, false);
    for span in &mut line.spans {
        span.italic = true;
    }
    // render wrapped with available_width = width - 2
}
```

- [ ] **Step 6: Update existing tests that check old inline code color**

In `tests/tui/markdown_test.rs`, update `renders_inline_code_with_color`:

```rust
#[test]
fn renders_inline_code_with_color() {
    let nodes = parse_markdown("use `cargo`");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let code_cell = (0..area.width).map(|x| buf.get(x, 0)).find(|c| c.ch == 'c');
    let cell = code_cell.expect("should find 'c' from 'cargo'");
    assert!(cell.fg.is_some(), "inline code should have a foreground color");
    use viv::core::terminal::style::Color;
    assert_eq!(cell.fg, Some(Color::Rgb(230, 150, 100)));
}
```

Update `inline_code_uses_claude_color` in the backward-compat section:

```rust
#[test]
fn inline_code_uses_new_orange_color() {
    use viv::core::terminal::style::Color;
    let lines = render_markdown("use `cargo test` to run");
    let code_span = lines[0]
        .spans
        .iter()
        .find(|s| s.text.contains("cargo test"))
        .unwrap();
    assert_eq!(
        code_span.fg,
        Some(Color::Rgb(230, 150, 100)),
        "inline code should use new warm orange"
    );
}
```

- [ ] **Step 7: Run all markdown tests**

Run: `cargo test --test tui_tests -- markdown_test`
Expected: all PASS.

- [ ] **Step 8: Commit**

```bash
git add src/tui/markdown.rs tests/tui/markdown_test.rs
git commit -m "feat(tui): heading levels, inline code restyle, italic, quote styling"
```

---

### Task 5: Code Block Background

**Files:**
- Modify: `src/tui/code_block.rs`
- Modify: `tests/tui/code_block_test.rs`

- [ ] **Step 1: Write failing test for code block background**

In `tests/tui/code_block_test.rs`, add:

```rust
#[test]
fn code_block_inner_has_background() {
    use viv::core::terminal::style::Color;
    let widget = CodeBlockWidget::new("let x = 1;", Some("rust"));
    let mut buf = make_buf(20, 5);
    widget.render(Rect::new(0, 0, 20, 5), &mut buf);
    // Inner cell at (1, 1) should have dark background
    let cell = buf.get(1, 1);
    assert_eq!(cell.bg, Some(Color::Rgb(30, 30, 30)), "code block inner should have dark bg");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test tui_tests -- code_block_test::code_block_inner_has_background`
Expected: FAIL — bg is None.

- [ ] **Step 3: Fill inner area background before rendering tokens**

In `src/tui/code_block.rs`, in the `render` method, after computing `inner` and before the code line loop, fill all inner cells with bg:

```rust
        let inner = block.inner(area);
        if inner.is_empty() {
            return;
        }

        // Fill inner area with dark background
        let code_bg = Color::Rgb(30, 30, 30);
        for row in 0..inner.height {
            for col in 0..inner.width {
                buf.get_mut(inner.x + col, inner.y + row).bg = Some(code_bg);
            }
        }

        // Render each line of code (existing code follows)
```

Also set `bg` on each token cell so it persists after character rendering:

```rust
                    let cell = buf.get_mut(x, y);
                    cell.ch = ch;
                    cell.fg = Some(fg);
                    cell.bg = Some(code_bg);
                    cell.bold = bold;
```

- [ ] **Step 4: Run all code_block tests**

Run: `cargo test --test tui_tests -- code_block_test`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tui/code_block.rs tests/tui/code_block_test.rs
git commit -m "feat(tui): dark background for code blocks"
```

---

### Task 6: UserMessage Word-Wrap

**Files:**
- Modify: `src/bus/terminal.rs`

- [ ] **Step 1: Update block_height_with_width for UserMessage**

In `src/bus/terminal.rs`, change the `UserMessage` arm in `block_height_with_width`:

```rust
ContentBlock::UserMessage { text } => {
    use crate::tui::paragraph::{Line, Span, wrap_line};
    let line = Line::from_spans(vec![Span::raw(text.clone())]);
    let effective_width = width.saturating_sub(2) as usize; // "> " prefix
    wrap_line(&line, effective_width.max(1)).len() as u16
}
```

- [ ] **Step 2: Update render_block for multi-row UserMessage**

In `src/bus/terminal.rs`, update the `UserMessage` arm in `render_block`:

```rust
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
        // First row starts after "> ", continuation rows indented 2 spaces
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
```

- [ ] **Step 3: Run cargo build**

Run: `cargo build`
Expected: compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add src/bus/terminal.rs
git commit -m "feat(tui): word-wrap for UserMessage display"
```

---

### Task 7: Welcome Screen — ContentBlock::Welcome + WelcomeWidget

**Files:**
- Modify: `src/tui/content.rs`
- Create: `src/tui/welcome.rs`
- Modify: `src/tui/mod.rs`
- Create: `tests/tui/welcome_test.rs`
- Modify: `tests/tui/mod.rs`
- Modify: `src/bus/terminal.rs`

- [ ] **Step 1: Add ContentBlock::Welcome variant**

In `src/tui/content.rs`, add to the `ContentBlock` enum:

```rust
    Welcome {
        model: Option<String>,
        cwd: String,
        branch: Option<String>,
    },
```

- [ ] **Step 2: Write failing tests for WelcomeWidget**

Create `tests/tui/welcome_test.rs`:

```rust
use viv::core::terminal::buffer::{Buffer, Rect};
use viv::core::terminal::style::Color;
use viv::tui::welcome::WelcomeWidget;
use viv::tui::widget::Widget;

#[test]
fn welcome_height_is_five() {
    assert_eq!(WelcomeWidget::HEIGHT, 5);
}

#[test]
fn welcome_renders_logo() {
    let widget = WelcomeWidget::new(
        Some("claude-sonnet-4-6"),
        "~/projects/viv",
        Some("main"),
    );
    let area = Rect::new(0, 0, 60, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // Logo second row starts with "_" somewhere in first 20 cols
    let row1: String = (0..20).map(|x| buf.get(x, 1).ch).collect();
    assert!(row1.contains('_'), "logo row 1 should contain '_': got '{row1}'");
}

#[test]
fn welcome_renders_model_info() {
    let widget = WelcomeWidget::new(
        Some("claude-sonnet-4-6"),
        "~/projects/viv",
        Some("main"),
    );
    let area = Rect::new(0, 0, 60, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // "Model:" label should appear in the right half
    let full: String = (0..60).map(|x| buf.get(x, 0).ch).collect();
    assert!(full.contains("Model"), "should contain Model label: got '{full}'");
}

#[test]
fn welcome_renders_placeholder_when_no_model() {
    let widget = WelcomeWidget::new(
        None,
        "~/projects/viv",
        Some("main"),
    );
    let area = Rect::new(0, 0, 60, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let full: String = (0..60).map(|x| buf.get(x, 0).ch).collect();
    assert!(full.contains("..."), "should show '...' when model unknown: got '{full}'");
}

#[test]
fn welcome_renders_cwd_info() {
    let widget = WelcomeWidget::new(
        Some("test-model"),
        "~/my/path",
        None,
    );
    let area = Rect::new(0, 0, 60, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let row1: String = (0..60).map(|x| buf.get(x, 1).ch).collect();
    assert!(row1.contains("~/my/path"), "should show CWD: got '{row1}'");
}

#[test]
fn welcome_logo_uses_claude_color() {
    let widget = WelcomeWidget::new(Some("m"), "~", None);
    let area = Rect::new(0, 0, 60, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // Find a non-space logo char and check its color
    let logo_cell = (0..20).map(|x| buf.get(x, 1))
        .find(|c| c.ch != ' ');
    if let Some(cell) = logo_cell {
        assert_eq!(cell.fg, Some(Color::Rgb(215, 119, 87)), "logo should use CLAUDE orange");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --test tui_tests -- welcome_test`
Expected: compilation error — `welcome` module does not exist.

- [ ] **Step 4: Register the module**

In `src/tui/mod.rs`, add:

```rust
pub mod welcome;
```

In `tests/tui/mod.rs`, add:

```rust
mod welcome_test;
```

- [ ] **Step 5: Implement WelcomeWidget**

Create `src/tui/welcome.rs`:

```rust
use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::style::theme;
use crate::tui::widget::Widget;

const LOGO: [&str; 5] = [
    "       _       ",
    "__   _(_)_   __",
    "\\ \\ / / \\ \\ / /",
    " \\ V /| |\\ V / ",
    "  \\_/ |_| \\_/  ",
];

const LOGO_WIDTH: u16 = 15;
const GAP: u16 = 4;

pub struct WelcomeWidget<'a> {
    model: Option<&'a str>,
    cwd: &'a str,
    branch: Option<&'a str>,
}

impl<'a> WelcomeWidget<'a> {
    pub const HEIGHT: u16 = 5;

    pub fn new(
        model: Option<&'a str>,
        cwd: &'a str,
        branch: Option<&'a str>,
    ) -> Self {
        WelcomeWidget { model, cwd, branch }
    }

    fn info_lines(&self) -> [(&str, String); 5] {
        let model_val = self.model.unwrap_or("...").to_string();
        let cwd_val = self.cwd.to_string();
        let branch_val = self.branch.unwrap_or("-").to_string();

        let platform = format!("{} {}",
            std::env::consts::OS,
            std::env::consts::ARCH,
        );

        let shell = std::env::var("SHELL")
            .ok()
            .and_then(|s| s.rsplit('/').next().map(|n| n.to_string()))
            .unwrap_or_else(|| "-".to_string());

        [
            ("Model:", model_val),
            ("CWD:", cwd_val),
            ("Branch:", branch_val),
            ("Platform:", platform),
            ("Shell:", shell),
        ]
    }
}

impl<'a> Widget for WelcomeWidget<'a> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() || area.height < Self::HEIGHT {
            return;
        }

        // Render logo
        for (row, line) in LOGO.iter().enumerate() {
            let y = area.y + row as u16;
            if y >= area.y + area.height {
                break;
            }
            buf.set_str(area.x, y, line, Some(theme::CLAUDE), false);
        }

        // Render info to the right of logo
        let info_x = area.x + LOGO_WIDTH + GAP;
        if info_x >= area.x + area.width {
            return; // Not enough room for info
        }

        let label_width: u16 = 10; // "Platform: " is the longest at 10 chars
        let info_lines = self.info_lines();

        for (row, (label, value)) in info_lines.iter().enumerate() {
            let y = area.y + row as u16;
            if y >= area.y + area.height {
                break;
            }

            // Label in CLAUDE orange, bold
            buf.set_str(info_x, y, label, Some(theme::CLAUDE), true);

            // Value in white
            let val_x = info_x + label_width;
            if val_x < area.x + area.width {
                buf.set_str(val_x, y, value, Some(theme::TEXT), false);
            }
        }
    }
}
```

- [ ] **Step 6: Run welcome tests**

Run: `cargo test --test tui_tests -- welcome_test`
Expected: all PASS.

- [ ] **Step 7: Commit**

```bash
git add src/tui/content.rs src/tui/welcome.rs src/tui/mod.rs tests/tui/welcome_test.rs tests/tui/mod.rs
git commit -m "feat(tui): neofetch-style WelcomeWidget with ASCII art logo"
```

---

### Task 8: Wire Welcome Screen into TerminalUI

**Files:**
- Modify: `src/bus/terminal.rs`

- [ ] **Step 1: Replace old welcome init with ContentBlock::Welcome**

In `src/bus/terminal.rs`, in `TerminalUI::new`, replace the welcome message construction (lines that push `ContentBlock::Markdown` with welcome_nodes and the empty-line separator) with:

```rust
        // Push welcome screen as first content block
        blocks.push(ContentBlock::Welcome {
            model: None,
            cwd: header.cwd.clone(),
            branch: header.branch.clone(),
        });
        conversation_state.append_item_height(5); // WelcomeWidget::HEIGHT

        // Empty line separator after welcome
        blocks.push(ContentBlock::Markdown {
            nodes: vec![MarkdownNode::Paragraph {
                spans: vec![crate::tui::content::InlineSpan::Text(String::new())],
            }],
        });
        conversation_state.append_item_height(1);
```

- [ ] **Step 2: Update handle_agent_message for Ready to update Welcome block**

In the `AgentMessage::Ready { model }` handler:

```rust
            AgentMessage::Ready { model } => {
                self.model_name = model.clone();
                // Update the Welcome block's model field
                if let Some(ContentBlock::Welcome { model: m, .. }) = self.blocks.first_mut() {
                    *m = Some(model);
                }
            }
```

- [ ] **Step 3: Update block_height_with_width for Welcome**

```rust
        ContentBlock::Welcome { .. } => crate::tui::welcome::WelcomeWidget::HEIGHT,
```

- [ ] **Step 4: Update render_block for Welcome**

In the `render_block` function, add a match arm:

```rust
        ContentBlock::Welcome { model, cwd, branch } => {
            use crate::tui::welcome::WelcomeWidget;
            let widget = WelcomeWidget::new(
                model.as_deref(),
                cwd,
                branch.as_deref(),
            );
            widget.render(area, buf);
        }
```

- [ ] **Step 5: Remove unused imports**

Remove the `format_welcome` import since it's no longer used.

- [ ] **Step 6: Run cargo build**

Run: `cargo build`
Expected: compiles without errors.

- [ ] **Step 7: Run cargo test**

Run: `cargo test`
Expected: all tests PASS. The `message_style_test` tests for `format_welcome` still pass since the function still exists, it's just not used by TerminalUI anymore.

- [ ] **Step 8: Commit**

```bash
git add src/bus/terminal.rs
git commit -m "feat(tui): wire neofetch welcome screen into TerminalUI"
```

---

### Task 9: Final Integration Verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: all tests PASS.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy`
Expected: no warnings.

- [ ] **Step 3: Run fmt check**

Run: `cargo fmt --check`
Expected: no formatting issues (run `cargo fmt` if needed).

- [ ] **Step 4: Manual smoke test**

Run: `cargo run` (requires VIV_API_KEY)

Verify:
1. Neofetch-style welcome screen shows logo + system info
2. Model name fills in after connection
3. Long assistant responses word-wrap correctly
4. Code blocks have dark background
5. Inline code has distinct warm orange + subtle bg
6. Headings show level distinction (h1 orange, h2 white, h3 gray)
7. Italic text renders with actual italic
8. Block quotes show italic + preserved inline styles
9. Spacing between paragraphs/sections
10. Long user messages word-wrap

- [ ] **Step 5: Final commit if any fixes needed**

```bash
git add -A
git commit -m "fix(tui): address integration issues from smoke test"
```
