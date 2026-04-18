# TUI Widget Framework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the TerminalUI from flat `Vec<Line>` rendering to a Widget-based conversation UI with Markdown rendering, syntax-highlighted code blocks, foldable tool calls, and virtual scrolling.

**Architecture:** ContentBlock data model drives a Widget tree. Agent messages are parsed into structured ContentBlocks (Markdown, CodeBlock, ToolCall, UserMessage). Each block type has a dedicated Widget with its own state. ConversationWidget provides virtual scrolling over the block list. A FocusManager enables Browse mode for navigating and expanding/collapsing tool calls.

**Tech Stack:** Rust (edition 2024), zero dependencies, existing custom TUI framework (`src/tui/`), existing Buffer/Cell rendering with diff-based updates.

**Spec:** `docs/superpowers/specs/2026-04-18-tui-widget-framework-design.md`

---

## File Map

### New Files

| File | Responsibility |
|------|---------------|
| `src/tui/content.rs` | `ContentBlock`, `MarkdownNode`, `InlineSpan` data types + `MarkdownParseBuffer` for streaming |
| `src/tui/syntax.rs` | `TokenKind` enum, `Token` struct, `Tokenizer` state machine |
| `src/tui/lang_profiles.rs` | `LangProfile` struct, 9 static profiles, `select_profile()` |
| `src/tui/code_block.rs` | `CodeBlockWidget` — renders highlighted code inside bordered block |
| `src/tui/tool_call.rs` | `ToolCallWidget` — foldable tool call display, `ToolCallState`, `ToolStatus` |
| `src/tui/focus.rs` | `FocusManager`, `UIMode` enum |
| `src/tui/conversation.rs` | `ConversationWidget`, `ConversationState` — virtual scrolling container |
| `tests/tui/content_test.rs` | Tests for ContentBlock parsing, MarkdownNode, InlineSpan, streaming buffer |
| `tests/tui/syntax_test.rs` | Tests for tokenizer state machine |
| `tests/tui/lang_profiles_test.rs` | Tests for language profile selection and keyword coverage |
| `tests/tui/code_block_test.rs` | Tests for code block widget rendering |
| `tests/tui/tool_call_test.rs` | Tests for tool call widget (folded/expanded) |
| `tests/tui/focus_test.rs` | Tests for focus manager navigation |
| `tests/tui/conversation_test.rs` | Tests for virtual scrolling, height calculation |

### Modified Files

| File | Changes |
|------|---------|
| `src/tui/mod.rs` | Add 5 new module declarations |
| `src/tui/markdown.rs` | Rewrite: replace `render_markdown()` with `MarkdownBlock` widget that renders `Vec<MarkdownNode>` |
| `src/bus/terminal.rs` | Major refactor: replace `history_lines` with `Vec<ContentBlock>`, add `MarkdownParseBuffer`, wire new widgets, add Browse mode |
| `tests/tui/mod.rs` | Add 7 new test module declarations |
| `tests/tui/markdown_test.rs` | Update tests for new Markdown data model |

---

## Task 1: Content Data Model

**Files:**
- Create: `src/tui/content.rs`
- Create: `tests/tui/content_test.rs`
- Modify: `src/tui/mod.rs`
- Modify: `tests/tui/mod.rs`

- [ ] **Step 1: Write failing tests for InlineSpan parsing**

```rust
// tests/tui/content_test.rs
use viv::tui::content::{parse_inline, InlineSpan};

#[test]
fn parse_plain_text() {
    let spans = parse_inline("hello world");
    assert_eq!(spans, vec![InlineSpan::Text("hello world".into())]);
}

#[test]
fn parse_bold_text() {
    let spans = parse_inline("hello **world** end");
    assert_eq!(spans, vec![
        InlineSpan::Text("hello ".into()),
        InlineSpan::Bold("world".into()),
        InlineSpan::Text(" end".into()),
    ]);
}

#[test]
fn parse_italic_text() {
    let spans = parse_inline("hello *world* end");
    assert_eq!(spans, vec![
        InlineSpan::Text("hello ".into()),
        InlineSpan::Italic("world".into()),
        InlineSpan::Text(" end".into()),
    ]);
}

#[test]
fn parse_inline_code() {
    let spans = parse_inline("use `cargo test` here");
    assert_eq!(spans, vec![
        InlineSpan::Text("use ".into()),
        InlineSpan::Code("cargo test".into()),
        InlineSpan::Text(" here".into()),
    ]);
}

#[test]
fn parse_link() {
    let spans = parse_inline("see [docs](https://example.com) here");
    assert_eq!(spans, vec![
        InlineSpan::Text("see ".into()),
        InlineSpan::Link { text: "docs".into(), url: "https://example.com".into() },
        InlineSpan::Text(" here".into()),
    ]);
}

#[test]
fn parse_mixed_inline() {
    let spans = parse_inline("**bold** and `code`");
    assert_eq!(spans, vec![
        InlineSpan::Bold("bold".into()),
        InlineSpan::Text(" and ".into()),
        InlineSpan::Code("code".into()),
    ]);
}

#[test]
fn parse_unclosed_bold() {
    let spans = parse_inline("hello **world");
    // unclosed bold should not drop content
    let has_world = spans.iter().any(|s| match s {
        InlineSpan::Text(t) | InlineSpan::Bold(t) => t.contains("world"),
        _ => false,
    });
    assert!(has_world);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test content_test 2>&1 | head -20`
Expected: compilation error — module `content` not found

- [ ] **Step 3: Implement content data model and InlineSpan parser**

```rust
// src/tui/content.rs

/// Inline text spans within a Markdown block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineSpan {
    Text(String),
    Bold(String),
    Italic(String),
    Code(String),
    Link { text: String, url: String },
}

/// Block-level Markdown nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownNode {
    Heading { level: u8, text: Vec<InlineSpan> },
    Paragraph { spans: Vec<InlineSpan> },
    List { ordered: bool, items: Vec<Vec<InlineSpan>> },
    Quote { spans: Vec<InlineSpan> },
    CodeBlock { language: Option<String>, code: String },
    HorizontalRule,
}

/// Top-level content blocks in a conversation.
#[derive(Debug, Clone)]
pub enum ContentBlock {
    UserMessage { text: String },
    Markdown { nodes: Vec<MarkdownNode> },
    CodeBlock { language: Option<String>, code: String },
    ToolCall {
        id: usize,
        name: String,
        input: String,
        output: Option<String>,
        error: Option<String>,
    },
}

/// Parse inline Markdown formatting into spans.
pub fn parse_inline(line: &str) -> Vec<InlineSpan> {
    let mut spans = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut buf = String::new();

    while i < len {
        // **bold**
        if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
            if !buf.is_empty() {
                spans.push(InlineSpan::Text(std::mem::take(&mut buf)));
            }
            i += 2;
            let mut inner = String::new();
            while i < len {
                if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
                    i += 2;
                    break;
                }
                inner.push(chars[i]);
                i += 1;
            }
            spans.push(InlineSpan::Bold(inner));
        }
        // *italic* (single star, not followed by another star)
        else if chars[i] == '*' && (i + 1 >= len || chars[i + 1] != '*') {
            if !buf.is_empty() {
                spans.push(InlineSpan::Text(std::mem::take(&mut buf)));
            }
            i += 1;
            let mut inner = String::new();
            while i < len && chars[i] != '*' {
                inner.push(chars[i]);
                i += 1;
            }
            if i < len {
                i += 1; // consume closing *
            }
            spans.push(InlineSpan::Italic(inner));
        }
        // `code`
        else if chars[i] == '`' {
            if !buf.is_empty() {
                spans.push(InlineSpan::Text(std::mem::take(&mut buf)));
            }
            i += 1;
            let mut inner = String::new();
            while i < len && chars[i] != '`' {
                inner.push(chars[i]);
                i += 1;
            }
            if i < len {
                i += 1; // consume closing `
            }
            spans.push(InlineSpan::Code(inner));
        }
        // [text](url)
        else if chars[i] == '[' {
            // look ahead for ](url)
            let mut text = String::new();
            let mut j = i + 1;
            while j < len && chars[j] != ']' {
                text.push(chars[j]);
                j += 1;
            }
            if j + 1 < len && chars[j] == ']' && chars[j + 1] == '(' {
                let mut url = String::new();
                let mut k = j + 2;
                while k < len && chars[k] != ')' {
                    url.push(chars[k]);
                    k += 1;
                }
                if k < len {
                    if !buf.is_empty() {
                        spans.push(InlineSpan::Text(std::mem::take(&mut buf)));
                    }
                    spans.push(InlineSpan::Link { text, url });
                    i = k + 1;
                } else {
                    buf.push(chars[i]);
                    i += 1;
                }
            } else {
                buf.push(chars[i]);
                i += 1;
            }
        } else {
            buf.push(chars[i]);
            i += 1;
        }
    }

    if !buf.is_empty() {
        spans.push(InlineSpan::Text(buf));
    }
    if spans.is_empty() {
        spans.push(InlineSpan::Text(String::new()));
    }
    spans
}
```

- [ ] **Step 4: Add module declarations**

Add to `src/tui/mod.rs` line 1:
```rust
pub mod content;
```

Add to `tests/tui/mod.rs`:
```rust
mod content_test;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test content_test -v`
Expected: all 7 tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/tui/content.rs src/tui/mod.rs tests/tui/content_test.rs tests/tui/mod.rs
git commit -m "feat(tui): add ContentBlock data model and InlineSpan parser"
```

---

## Task 2: Markdown Block Parser

**Files:**
- Modify: `src/tui/content.rs`
- Modify: `tests/tui/content_test.rs`

- [ ] **Step 1: Write failing tests for block-level Markdown parsing**

Append to `tests/tui/content_test.rs`:

```rust
use viv::tui::content::{parse_markdown, MarkdownNode};

#[test]
fn parse_heading() {
    let nodes = parse_markdown("# Hello World");
    assert_eq!(nodes.len(), 1);
    match &nodes[0] {
        MarkdownNode::Heading { level, text } => {
            assert_eq!(*level, 1);
            assert_eq!(text.len(), 1);
        }
        _ => panic!("expected Heading"),
    }
}

#[test]
fn parse_h2_heading() {
    let nodes = parse_markdown("## Section");
    match &nodes[0] {
        MarkdownNode::Heading { level, .. } => assert_eq!(*level, 2),
        _ => panic!("expected Heading"),
    }
}

#[test]
fn parse_unordered_list() {
    let nodes = parse_markdown("- item one\n- item two");
    assert_eq!(nodes.len(), 1);
    match &nodes[0] {
        MarkdownNode::List { ordered, items } => {
            assert!(!ordered);
            assert_eq!(items.len(), 2);
        }
        _ => panic!("expected List"),
    }
}

#[test]
fn parse_ordered_list() {
    let nodes = parse_markdown("1. first\n2. second");
    assert_eq!(nodes.len(), 1);
    match &nodes[0] {
        MarkdownNode::List { ordered, items } => {
            assert!(ordered);
            assert_eq!(items.len(), 2);
        }
        _ => panic!("expected List"),
    }
}

#[test]
fn parse_quote_block() {
    let nodes = parse_markdown("> quoted text");
    assert_eq!(nodes.len(), 1);
    match &nodes[0] {
        MarkdownNode::Quote { .. } => {}
        _ => panic!("expected Quote"),
    }
}

#[test]
fn parse_code_block() {
    let nodes = parse_markdown("```rust\nfn main() {}\n```");
    assert_eq!(nodes.len(), 1);
    match &nodes[0] {
        MarkdownNode::CodeBlock { language, code } => {
            assert_eq!(language.as_deref(), Some("rust"));
            assert_eq!(code, "fn main() {}");
        }
        _ => panic!("expected CodeBlock"),
    }
}

#[test]
fn parse_horizontal_rule() {
    let nodes = parse_markdown("---");
    assert_eq!(nodes.len(), 1);
    assert!(matches!(nodes[0], MarkdownNode::HorizontalRule));
}

#[test]
fn parse_paragraph() {
    let nodes = parse_markdown("just some text");
    assert_eq!(nodes.len(), 1);
    assert!(matches!(nodes[0], MarkdownNode::Paragraph { .. }));
}

#[test]
fn parse_mixed_blocks() {
    let md = "# Title\n\nSome text.\n\n- a\n- b\n\n```\ncode\n```";
    let nodes = parse_markdown(md);
    assert!(nodes.len() >= 4);
    assert!(matches!(nodes[0], MarkdownNode::Heading { .. }));
    assert!(matches!(nodes[1], MarkdownNode::Paragraph { .. }));
    assert!(matches!(nodes[2], MarkdownNode::List { .. }));
    assert!(matches!(nodes[3], MarkdownNode::CodeBlock { .. }));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test content_test parse_heading 2>&1 | head -10`
Expected: `parse_markdown` not found

- [ ] **Step 3: Implement `parse_markdown`**

Add to `src/tui/content.rs`:

```rust
/// Parse Markdown text into a list of block-level nodes.
pub fn parse_markdown(text: &str) -> Vec<MarkdownNode> {
    let mut nodes = Vec::new();
    let lines: Vec<&str> = text.split('\n').collect();
    let len = lines.len();
    let mut i = 0;

    while i < len {
        let line = lines[i].trim_end();

        // Empty line — skip
        if line.is_empty() {
            i += 1;
            continue;
        }

        // Fenced code block
        if line.starts_with("```") {
            let lang = line[3..].trim();
            let language = if lang.is_empty() { None } else { Some(lang.to_string()) };
            let mut code_lines = Vec::new();
            i += 1;
            while i < len && !lines[i].trim_end().starts_with("```") {
                code_lines.push(lines[i].trim_end());
                i += 1;
            }
            if i < len { i += 1; } // skip closing ```
            nodes.push(MarkdownNode::CodeBlock {
                language,
                code: code_lines.join("\n"),
            });
            continue;
        }

        // Horizontal rule
        if line == "---" || line == "***" || line == "___" {
            nodes.push(MarkdownNode::HorizontalRule);
            i += 1;
            continue;
        }

        // Heading
        if line.starts_with('#') {
            let mut level: u8 = 0;
            for ch in line.chars() {
                if ch == '#' { level += 1; } else { break; }
            }
            if level <= 6 && line.len() > level as usize && line.as_bytes()[level as usize] == b' ' {
                let text_part = &line[(level as usize + 1)..];
                nodes.push(MarkdownNode::Heading {
                    level,
                    text: parse_inline(text_part),
                });
                i += 1;
                continue;
            }
        }

        // Quote
        if let Some(rest) = line.strip_prefix("> ") {
            nodes.push(MarkdownNode::Quote {
                spans: parse_inline(rest),
            });
            i += 1;
            continue;
        }

        // Unordered list
        if line.starts_with("- ") || line.starts_with("* ") {
            let mut items = Vec::new();
            while i < len {
                let l = lines[i].trim_end();
                if let Some(rest) = l.strip_prefix("- ").or_else(|| l.strip_prefix("* ")) {
                    items.push(parse_inline(rest));
                    i += 1;
                } else {
                    break;
                }
            }
            nodes.push(MarkdownNode::List { ordered: false, items });
            continue;
        }

        // Ordered list
        if is_ordered_list_line(line) {
            let mut items = Vec::new();
            while i < len {
                let l = lines[i].trim_end();
                if is_ordered_list_line(l) {
                    let dot_pos = l.find(". ").unwrap();
                    items.push(parse_inline(&l[dot_pos + 2..]));
                    i += 1;
                } else {
                    break;
                }
            }
            nodes.push(MarkdownNode::List { ordered: true, items });
            continue;
        }

        // Paragraph (default)
        nodes.push(MarkdownNode::Paragraph {
            spans: parse_inline(line),
        });
        i += 1;
    }

    nodes
}

fn is_ordered_list_line(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    i > 0 && i < bytes.len() && bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1] == b' '
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test content_test -v`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/tui/content.rs tests/tui/content_test.rs
git commit -m "feat(tui): add Markdown block-level parser"
```

---

## Task 3: Streaming Markdown Parse Buffer

**Files:**
- Modify: `src/tui/content.rs`
- Modify: `tests/tui/content_test.rs`

- [ ] **Step 1: Write failing tests for MarkdownParseBuffer**

Append to `tests/tui/content_test.rs`:

```rust
use viv::tui::content::{MarkdownParseBuffer, ContentBlock};

#[test]
fn stream_buffer_complete_line() {
    let mut buf = MarkdownParseBuffer::new();
    let blocks = buf.push("hello world\n");
    assert_eq!(blocks.len(), 1);
    assert!(matches!(blocks[0], ContentBlock::Markdown { .. }));
}

#[test]
fn stream_buffer_incomplete_line() {
    let mut buf = MarkdownParseBuffer::new();
    let blocks = buf.push("hello ");
    assert!(blocks.is_empty(), "incomplete line should not produce blocks");
    let blocks = buf.push("world\n");
    assert_eq!(blocks.len(), 1);
}

#[test]
fn stream_buffer_code_block() {
    let mut buf = MarkdownParseBuffer::new();
    assert!(buf.push("```rust\n").is_empty());
    assert!(buf.push("fn main() {}\n").is_empty());
    let blocks = buf.push("```\n");
    assert_eq!(blocks.len(), 1);
    assert!(matches!(blocks[0], ContentBlock::CodeBlock { .. }));
}

#[test]
fn stream_buffer_flush_pending() {
    let mut buf = MarkdownParseBuffer::new();
    buf.push("incomplete text");
    let blocks = buf.flush();
    assert_eq!(blocks.len(), 1);
}

#[test]
fn stream_buffer_code_block_promotes_to_content_block() {
    let mut buf = MarkdownParseBuffer::new();
    buf.push("text before\n");
    buf.push("```python\n");
    buf.push("print('hi')\n");
    let blocks = buf.push("```\n");
    // The code block should be promoted to ContentBlock::CodeBlock
    let has_code = blocks.iter().any(|b| matches!(b, ContentBlock::CodeBlock { .. }));
    assert!(has_code);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test content_test stream_buffer 2>&1 | head -10`
Expected: `MarkdownParseBuffer` not found

- [ ] **Step 3: Implement MarkdownParseBuffer**

Add to `src/tui/content.rs`:

```rust
/// Streaming Markdown parser that buffers incomplete input.
/// Each `push()` accepts a text chunk and returns any complete ContentBlocks.
pub struct MarkdownParseBuffer {
    buffer: String,
    in_code_block: bool,
    code_language: Option<String>,
    code_lines: Vec<String>,
}

impl MarkdownParseBuffer {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            in_code_block: false,
            code_language: None,
            code_lines: Vec::new(),
        }
    }

    /// Push a text chunk. Returns any ContentBlocks that are now complete.
    pub fn push(&mut self, chunk: &str) -> Vec<ContentBlock> {
        self.buffer.push_str(chunk);
        self.drain_complete_lines()
    }

    /// Flush any remaining buffered content as blocks.
    pub fn flush(&mut self) -> Vec<ContentBlock> {
        if self.in_code_block {
            // Unclosed code block — emit what we have
            let code = std::mem::take(&mut self.code_lines).join("\n");
            let language = self.code_language.take();
            self.in_code_block = false;
            return vec![ContentBlock::CodeBlock { language, code }];
        }
        if !self.buffer.is_empty() {
            let text = std::mem::take(&mut self.buffer);
            let nodes = parse_markdown(&text);
            if nodes.is_empty() {
                return Vec::new();
            }
            return vec![ContentBlock::Markdown { nodes }];
        }
        Vec::new()
    }

    fn drain_complete_lines(&mut self) -> Vec<ContentBlock> {
        let mut blocks = Vec::new();
        let mut ready_lines = Vec::new();

        loop {
            let newline_pos = match self.buffer.find('\n') {
                Some(pos) => pos,
                None => break,
            };
            let line: String = self.buffer[..newline_pos].to_string();
            self.buffer = self.buffer[newline_pos + 1..].to_string();

            if self.in_code_block {
                if line.trim_end().starts_with("```") {
                    // Close code block
                    let code = std::mem::take(&mut self.code_lines).join("\n");
                    let language = self.code_language.take();
                    self.in_code_block = false;
                    // Flush any ready lines before the code block as Markdown
                    if !ready_lines.is_empty() {
                        let text = ready_lines.join("\n");
                        let nodes = parse_markdown(&text);
                        if !nodes.is_empty() {
                            blocks.push(ContentBlock::Markdown { nodes });
                        }
                        ready_lines = Vec::new();
                    }
                    blocks.push(ContentBlock::CodeBlock { language, code });
                } else {
                    self.code_lines.push(line);
                }
            } else if line.trim_end().starts_with("```") {
                // Open code block — flush accumulated lines first
                if !ready_lines.is_empty() {
                    let text = ready_lines.join("\n");
                    let nodes = parse_markdown(&text);
                    if !nodes.is_empty() {
                        blocks.push(ContentBlock::Markdown { nodes });
                    }
                    ready_lines = Vec::new();
                }
                let lang = line.trim_end()[3..].trim();
                self.code_language = if lang.is_empty() { None } else { Some(lang.to_string()) };
                self.in_code_block = true;
            } else {
                ready_lines.push(line);
            }
        }

        // Flush completed non-code lines as Markdown
        if !ready_lines.is_empty() {
            let text = ready_lines.join("\n");
            let nodes = parse_markdown(&text);
            if !nodes.is_empty() {
                blocks.push(ContentBlock::Markdown { nodes });
            }
        }

        blocks
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test content_test -v`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/tui/content.rs tests/tui/content_test.rs
git commit -m "feat(tui): add streaming MarkdownParseBuffer"
```

---

## Task 4: Syntax Tokenizer

**Files:**
- Create: `src/tui/syntax.rs`
- Create: `tests/tui/syntax_test.rs`
- Modify: `src/tui/mod.rs`
- Modify: `tests/tui/mod.rs`

- [ ] **Step 1: Write failing tests for tokenizer**

```rust
// tests/tui/syntax_test.rs
use viv::tui::syntax::{tokenize, TokenKind};

#[test]
fn tokenize_keyword() {
    let tokens = tokenize("fn main", None);
    assert_eq!(tokens[0].kind, TokenKind::Keyword);
    assert_eq!(tokens[0].text, "fn");
}

#[test]
fn tokenize_string_double_quote() {
    let tokens = tokenize("\"hello\"", None);
    assert_eq!(tokens[0].kind, TokenKind::String);
    assert_eq!(tokens[0].text, "\"hello\"");
}

#[test]
fn tokenize_line_comment_slash() {
    let tokens = tokenize("// comment", None);
    assert_eq!(tokens[0].kind, TokenKind::Comment);
    assert_eq!(tokens[0].text, "// comment");
}

#[test]
fn tokenize_number() {
    let tokens = tokenize("42", None);
    assert_eq!(tokens[0].kind, TokenKind::Number);
    assert_eq!(tokens[0].text, "42");
}

#[test]
fn tokenize_hex_number() {
    let tokens = tokenize("0xFF", None);
    assert_eq!(tokens[0].kind, TokenKind::Number);
}

#[test]
fn tokenize_type_uppercase() {
    let tokens = tokenize("String", Some("rust"));
    assert_eq!(tokens[0].kind, TokenKind::Type);
}

#[test]
fn tokenize_function_call() {
    let tokens = tokenize("foo()", None);
    assert_eq!(tokens[0].kind, TokenKind::Function);
    assert_eq!(tokens[0].text, "foo");
}

#[test]
fn tokenize_operator() {
    let tokens = tokenize("a + b", None);
    assert_eq!(tokens[1].kind, TokenKind::Operator);
}

#[test]
fn tokenize_rust_lifetime() {
    let tokens = tokenize("'a", Some("rust"));
    assert_eq!(tokens[0].kind, TokenKind::Lifetime);
}

#[test]
fn tokenize_rust_attribute() {
    let tokens = tokenize("#[derive(Debug)]", Some("rust"));
    assert_eq!(tokens[0].kind, TokenKind::Attribute);
}

#[test]
fn tokenize_python_comment() {
    let tokens = tokenize("# comment", Some("python"));
    assert_eq!(tokens[0].kind, TokenKind::Comment);
}

#[test]
fn tokenize_python_hash_not_comment_in_rust() {
    let tokens = tokenize("#[test]", Some("rust"));
    // In Rust, # is attribute prefix, not comment
    assert_ne!(tokens[0].kind, TokenKind::Comment);
}

#[test]
fn tokenize_block_comment() {
    let tokens = tokenize("/* block */", Some("rust"));
    assert_eq!(tokens[0].kind, TokenKind::Comment);
}

#[test]
fn tokenize_python_triple_quote() {
    let tokens = tokenize("\"\"\"docstring\"\"\"", Some("python"));
    assert_eq!(tokens[0].kind, TokenKind::String);
}

#[test]
fn tokenize_js_template_literal() {
    let tokens = tokenize("`hello ${name}`", Some("js"));
    assert_eq!(tokens[0].kind, TokenKind::String);
}

#[test]
fn tokenize_mixed_rust_line() {
    let tokens = tokenize("fn main() { let x = 42; }", Some("rust"));
    let kinds: Vec<TokenKind> = tokens.iter().map(|t| t.kind).collect();
    assert_eq!(kinds[0], TokenKind::Keyword); // fn
    assert_eq!(kinds[2], TokenKind::Function); // main (if followed by parens... depends on impl)
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test syntax_test 2>&1 | head -10`
Expected: module `syntax` not found

- [ ] **Step 3: Implement TokenKind, Token, and tokenize()**

```rust
// src/tui/syntax.rs

/// Token classification for syntax highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Keyword,
    String,
    Comment,
    Number,
    Type,
    Function,
    Operator,
    Punctuation,
    Attribute,
    Lifetime,
    Plain,
}

/// A single token with its kind and text content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
}

use crate::tui::lang_profiles::{select_profile, LangProfile};

/// Tokenize a single line of code using the appropriate language profile.
pub fn tokenize(line: &str, language: Option<&str>) -> Vec<Token> {
    let profile = select_profile(language);
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        // Skip whitespace — emit as Plain
        if ch.is_whitespace() {
            let start = i;
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Plain,
                text: chars[start..i].iter().collect(),
            });
            continue;
        }

        // Line comments
        if let Some(token) = try_line_comment(&chars, i, profile) {
            tokens.push(token);
            break; // rest of line is comment
        }

        // Block comments
        if let Some((token, end)) = try_block_comment(&chars, i, profile) {
            tokens.push(token);
            i = end;
            continue;
        }

        // Attribute prefix
        if let Some((token, end)) = try_attribute(&chars, i, profile) {
            tokens.push(token);
            i = end;
            continue;
        }

        // Lifetime (Rust 'a)
        if profile.lifetime_prefix && ch == '\'' && i + 1 < len && chars[i + 1].is_alphabetic() {
            let start = i;
            i += 1; // skip '
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Lifetime,
                text: chars[start..i].iter().collect(),
            });
            continue;
        }

        // Strings
        if let Some((token, end)) = try_string(&chars, i, profile) {
            tokens.push(token);
            i = end;
            continue;
        }

        // Numbers
        if ch.is_ascii_digit() || (ch == '.' && i + 1 < len && chars[i + 1].is_ascii_digit()) {
            let start = i;
            if ch == '0' && i + 1 < len && (chars[i + 1] == 'x' || chars[i + 1] == 'X') {
                i += 2;
                while i < len && (chars[i].is_ascii_hexdigit() || chars[i] == '_') {
                    i += 1;
                }
            } else if ch == '0' && i + 1 < len && (chars[i + 1] == 'b' || chars[i + 1] == 'o') {
                i += 2;
                while i < len && (chars[i].is_ascii_digit() || chars[i] == '_') {
                    i += 1;
                }
            } else {
                while i < len && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '_' || chars[i] == 'e' || chars[i] == 'E') {
                    i += 1;
                }
            }
            // Type suffix (u32, i64, f64, etc.)
            while i < len && chars[i].is_alphabetic() {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Number,
                text: chars[start..i].iter().collect(),
            });
            continue;
        }

        // Identifiers and keywords
        if ch.is_alphabetic() || ch == '_' {
            let start = i;
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();

            // Check if followed by ( → function
            let mut j = i;
            while j < len && chars[j].is_whitespace() {
                j += 1;
            }
            let is_call = j < len && chars[j] == '(';

            let kind = if profile.keywords.contains(&word.as_str()) {
                TokenKind::Keyword
            } else if is_call {
                TokenKind::Function
            } else if profile.type_starts_upper && word.chars().next().is_some_and(|c| c.is_uppercase()) {
                TokenKind::Type
            } else {
                TokenKind::Plain
            };

            tokens.push(Token { kind, text: word });
            continue;
        }

        // Operators
        if is_operator(ch) {
            let start = i;
            // Multi-char operators: ::, ->, =>, !=, ==, <=, >=, &&, ||, <<, >>
            i += 1;
            if i < len && is_operator_pair(ch, chars[i]) {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Operator,
                text: chars[start..i].iter().collect(),
            });
            continue;
        }

        // Punctuation
        if is_punctuation(ch) {
            tokens.push(Token {
                kind: TokenKind::Punctuation,
                text: ch.to_string(),
            });
            i += 1;
            continue;
        }

        // Fallback: plain
        tokens.push(Token {
            kind: TokenKind::Plain,
            text: ch.to_string(),
        });
        i += 1;
    }

    tokens
}

fn try_line_comment(chars: &[char], i: usize, profile: &LangProfile) -> Option<Token> {
    let remaining: String = chars[i..].iter().collect();
    for prefix in profile.line_comments {
        if remaining.starts_with(prefix) {
            return Some(Token {
                kind: TokenKind::Comment,
                text: remaining,
            });
        }
    }
    None
}

fn try_block_comment(chars: &[char], i: usize, profile: &LangProfile) -> Option<(Token, usize)> {
    let (open, close) = profile.block_comment?;
    let remaining: String = chars[i..].iter().collect();
    if !remaining.starts_with(open) {
        return None;
    }
    let start = i;
    let mut j = i + open.len();
    let close_chars: Vec<char> = close.chars().collect();
    while j + close_chars.len() <= chars.len() {
        let slice: String = chars[j..j + close_chars.len()].iter().collect();
        if slice == close {
            j += close_chars.len();
            return Some((
                Token { kind: TokenKind::Comment, text: chars[start..j].iter().collect() },
                j,
            ));
        }
        j += 1;
    }
    // Unclosed block comment — consume rest of line
    Some((
        Token { kind: TokenKind::Comment, text: chars[start..].iter().collect() },
        chars.len(),
    ))
}

fn try_attribute(chars: &[char], i: usize, profile: &LangProfile) -> Option<(Token, usize)> {
    let prefix = profile.attribute_prefix?;
    if chars[i] != prefix {
        return None;
    }
    let start = i;
    let mut j = i + 1;
    if prefix == '#' {
        // Rust: #[...] or #![...]
        if j < chars.len() && (chars[j] == '[' || (chars[j] == '!' && j + 1 < chars.len() && chars[j + 1] == '[')) {
            let mut depth = 0;
            while j < chars.len() {
                if chars[j] == '[' { depth += 1; }
                if chars[j] == ']' {
                    depth -= 1;
                    if depth == 0 { j += 1; break; }
                }
                j += 1;
            }
        } else {
            return None; // not an attribute
        }
    } else {
        // @ prefix: @decorator, @Override
        while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_' || chars[j] == '.') {
            j += 1;
        }
    }
    Some((
        Token { kind: TokenKind::Attribute, text: chars[start..j].iter().collect() },
        j,
    ))
}

fn try_string(chars: &[char], i: usize, profile: &LangProfile) -> Option<(Token, usize)> {
    let ch = chars[i];

    // Template literals (JS backtick)
    if profile.template_literal && ch == '`' {
        let start = i;
        let mut j = i + 1;
        while j < chars.len() && chars[j] != '`' {
            if chars[j] == '\\' { j += 1; } // skip escaped
            j += 1;
        }
        if j < chars.len() { j += 1; }
        return Some((
            Token { kind: TokenKind::String, text: chars[start..j].iter().collect() },
            j,
        ));
    }

    // Check if this is a valid string quote for this language
    if !profile.string_quotes.contains(&ch) {
        return None;
    }

    // Triple quotes (Python)
    if profile.triple_quote && i + 2 < chars.len() && chars[i + 1] == ch && chars[i + 2] == ch {
        let start = i;
        let mut j = i + 3;
        while j + 2 < chars.len() {
            if chars[j] == ch && chars[j + 1] == ch && chars[j + 2] == ch {
                j += 3;
                return Some((
                    Token { kind: TokenKind::String, text: chars[start..j].iter().collect() },
                    j,
                ));
            }
            j += 1;
        }
        // Unclosed triple quote — consume rest
        return Some((
            Token { kind: TokenKind::String, text: chars[start..].iter().collect() },
            chars.len(),
        ));
    }

    // Raw strings (Rust r"...")
    if let Some(raw_prefix) = profile.raw_string {
        let remaining: String = chars[i..].iter().collect();
        if remaining.starts_with(raw_prefix) {
            let start = i;
            let mut j = i + raw_prefix.len();
            let quote = chars[j - 1]; // last char of prefix is the quote
            while j < chars.len() && chars[j] != quote {
                j += 1;
            }
            if j < chars.len() { j += 1; }
            return Some((
                Token { kind: TokenKind::String, text: chars[start..j].iter().collect() },
                j,
            ));
        }
    }

    // Regular string
    let start = i;
    let mut j = i + 1;
    while j < chars.len() && chars[j] != ch {
        if chars[j] == '\\' { j += 1; } // skip escaped char
        j += 1;
    }
    if j < chars.len() { j += 1; } // consume closing quote
    Some((
        Token { kind: TokenKind::String, text: chars[start..j].iter().collect() },
        j,
    ))
}

fn is_operator(ch: char) -> bool {
    matches!(ch, '=' | '+' | '-' | '*' | '/' | '<' | '>' | '!' | '&' | '|' | '^' | '%' | '~' | ':')
}

fn is_operator_pair(a: char, b: char) -> bool {
    matches!((a, b),
        (':', ':') | ('-', '>') | ('=', '>') | ('!', '=') | ('=', '=') |
        ('<', '=') | ('>', '=') | ('&', '&') | ('|', '|') | ('<', '<') |
        ('>', '>') | ('+', '=') | ('-', '=') | ('*', '=') | ('/', '=')
    )
}

fn is_punctuation(ch: char) -> bool {
    matches!(ch, '{' | '}' | '(' | ')' | '[' | ']' | ';' | ',' | '.')
}
```

- [ ] **Step 4: Add module declarations**

Add to `src/tui/mod.rs`:
```rust
pub mod syntax;
```

Add to `tests/tui/mod.rs`:
```rust
mod syntax_test;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test syntax_test -v`
Expected: all tests PASS (may need `lang_profiles` first — see Task 5)

Note: This task depends on Task 5 (lang_profiles) compiling. Implement them together or stub `lang_profiles` first.

- [ ] **Step 6: Commit**

```bash
git add src/tui/syntax.rs src/tui/mod.rs tests/tui/syntax_test.rs tests/tui/mod.rs
git commit -m "feat(tui): add syntax tokenizer state machine"
```

---

## Task 5: Language Profiles

**Files:**
- Create: `src/tui/lang_profiles.rs`
- Create: `tests/tui/lang_profiles_test.rs`
- Modify: `src/tui/mod.rs`
- Modify: `tests/tui/mod.rs`

Note: Implement this task **before or alongside Task 4** since the tokenizer imports from it.

- [ ] **Step 1: Write failing tests**

```rust
// tests/tui/lang_profiles_test.rs
use viv::tui::lang_profiles::{select_profile, LangProfile};

#[test]
fn select_rust_profile() {
    let p = select_profile(Some("rust"));
    assert_eq!(p.name, "rust");
    assert!(p.keywords.contains(&"fn"));
    assert!(p.lifetime_prefix);
}

#[test]
fn select_rs_alias() {
    let p = select_profile(Some("rs"));
    assert_eq!(p.name, "rust");
}

#[test]
fn select_python_profile() {
    let p = select_profile(Some("python"));
    assert_eq!(p.name, "python");
    assert!(p.keywords.contains(&"def"));
    assert!(p.triple_quote);
    assert_eq!(p.line_comments, &["#"]);
}

#[test]
fn select_js_profile() {
    let p = select_profile(Some("js"));
    assert_eq!(p.name, "javascript");
    assert!(p.template_literal);
}

#[test]
fn select_typescript_alias() {
    let p = select_profile(Some("typescript"));
    assert_eq!(p.name, "javascript");
}

#[test]
fn select_go_profile() {
    let p = select_profile(Some("go"));
    assert_eq!(p.name, "go");
    assert!(p.type_starts_upper);
}

#[test]
fn select_shell_profile() {
    let p = select_profile(Some("bash"));
    assert_eq!(p.name, "shell");
    assert!(p.keywords.contains(&"fi"));
}

#[test]
fn select_json_profile() {
    let p = select_profile(Some("json"));
    assert_eq!(p.name, "json");
    assert!(p.line_comments.is_empty());
}

#[test]
fn select_unknown_returns_generic() {
    let p = select_profile(Some("brainfuck"));
    assert_eq!(p.name, "generic");
}

#[test]
fn select_none_returns_generic() {
    let p = select_profile(None);
    assert_eq!(p.name, "generic");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test lang_profiles_test 2>&1 | head -10`
Expected: module not found

- [ ] **Step 3: Implement LangProfile and all profiles**

```rust
// src/tui/lang_profiles.rs

/// Language-specific configuration for the syntax tokenizer.
pub struct LangProfile {
    pub name: &'static str,
    pub keywords: &'static [&'static str],
    pub line_comments: &'static [&'static str],
    pub block_comment: Option<(&'static str, &'static str)>,
    pub string_quotes: &'static [char],
    pub raw_string: Option<&'static str>,
    pub triple_quote: bool,
    pub template_literal: bool,
    pub type_starts_upper: bool,
    pub lifetime_prefix: bool,
    pub attribute_prefix: Option<char>,
}

pub static RUST_PROFILE: LangProfile = LangProfile {
    name: "rust",
    keywords: &[
        "fn", "let", "mut", "pub", "struct", "enum", "impl", "trait", "use", "mod",
        "async", "await", "self", "Self", "crate", "super", "where", "type", "const",
        "static", "ref", "match", "if", "else", "for", "while", "loop", "break",
        "continue", "return", "in", "unsafe", "extern", "move", "dyn", "as",
        "true", "false",
    ],
    line_comments: &["//"],
    block_comment: Some(("/*", "*/")),
    string_quotes: &['"'],
    raw_string: Some("r\""),
    triple_quote: false,
    template_literal: false,
    type_starts_upper: true,
    lifetime_prefix: true,
    attribute_prefix: Some('#'),
};

pub static PYTHON_PROFILE: LangProfile = LangProfile {
    name: "python",
    keywords: &[
        "def", "class", "import", "from", "as", "with", "try", "except", "raise",
        "lambda", "yield", "if", "elif", "else", "for", "while", "break", "continue",
        "return", "in", "not", "and", "or", "is", "None", "True", "False", "pass",
        "del", "global", "nonlocal", "assert", "async", "await",
    ],
    line_comments: &["#"],
    block_comment: None,
    string_quotes: &['"', '\''],
    raw_string: None,
    triple_quote: true,
    template_literal: false,
    type_starts_upper: false,
    lifetime_prefix: false,
    attribute_prefix: Some('@'),
};

pub static JS_PROFILE: LangProfile = LangProfile {
    name: "javascript",
    keywords: &[
        "function", "const", "let", "var", "export", "default", "import", "async",
        "await", "typeof", "if", "else", "for", "while", "do", "switch", "case",
        "break", "continue", "return", "in", "of", "new", "this", "class", "extends",
        "super", "try", "catch", "finally", "throw", "yield", "true", "false", "null",
        "undefined", "void", "delete", "instanceof",
    ],
    line_comments: &["//"],
    block_comment: Some(("/*", "*/")),
    string_quotes: &['"', '\''],
    raw_string: None,
    triple_quote: false,
    template_literal: true,
    type_starts_upper: true,
    lifetime_prefix: false,
    attribute_prefix: None,
};

pub static GO_PROFILE: LangProfile = LangProfile {
    name: "go",
    keywords: &[
        "func", "package", "import", "type", "struct", "interface", "map", "chan",
        "select", "if", "else", "for", "range", "switch", "case", "default", "break",
        "continue", "return", "go", "defer", "var", "const", "true", "false", "nil",
    ],
    line_comments: &["//"],
    block_comment: Some(("/*", "*/")),
    string_quotes: &['"', '`'],
    raw_string: None,
    triple_quote: false,
    template_literal: false,
    type_starts_upper: true,
    lifetime_prefix: false,
    attribute_prefix: None,
};

pub static JAVA_C_PROFILE: LangProfile = LangProfile {
    name: "java_c",
    keywords: &[
        "class", "interface", "extends", "implements", "abstract", "final", "static",
        "void", "if", "else", "for", "while", "do", "switch", "case", "break",
        "continue", "return", "new", "this", "super", "try", "catch", "finally",
        "throw", "throws", "public", "private", "protected", "import", "package",
        "true", "false", "null", "int", "float", "double", "char", "boolean", "long",
        "short", "byte", "typedef", "sizeof", "struct", "union", "enum", "unsigned",
        "signed", "const", "volatile", "extern", "register", "auto",
    ],
    line_comments: &["//"],
    block_comment: Some(("/*", "*/")),
    string_quotes: &['"', '\''],
    raw_string: None,
    triple_quote: false,
    template_literal: false,
    type_starts_upper: true,
    lifetime_prefix: false,
    attribute_prefix: Some('@'),
};

pub static SHELL_PROFILE: LangProfile = LangProfile {
    name: "shell",
    keywords: &[
        "if", "then", "elif", "else", "fi", "for", "while", "do", "done", "case",
        "esac", "in", "function", "local", "export", "return", "exit", "echo", "read",
        "true", "false",
    ],
    line_comments: &["#"],
    block_comment: None,
    string_quotes: &['"', '\''],
    raw_string: None,
    triple_quote: false,
    template_literal: false,
    type_starts_upper: false,
    lifetime_prefix: false,
    attribute_prefix: None,
};

pub static JSON_PROFILE: LangProfile = LangProfile {
    name: "json",
    keywords: &["true", "false", "null"],
    line_comments: &[],
    block_comment: None,
    string_quotes: &['"'],
    raw_string: None,
    triple_quote: false,
    template_literal: false,
    type_starts_upper: false,
    lifetime_prefix: false,
    attribute_prefix: None,
};

pub static YAML_TOML_PROFILE: LangProfile = LangProfile {
    name: "yaml_toml",
    keywords: &["true", "false", "null", "yes", "no", "on", "off"],
    line_comments: &["#"],
    block_comment: None,
    string_quotes: &['"', '\''],
    raw_string: None,
    triple_quote: false,
    template_literal: false,
    type_starts_upper: false,
    lifetime_prefix: false,
    attribute_prefix: None,
};

pub static GENERIC_PROFILE: LangProfile = LangProfile {
    name: "generic",
    keywords: &[
        "if", "else", "for", "while", "return", "break", "continue", "match", "switch",
        "case", "fn", "function", "def", "class", "struct", "enum", "impl", "trait",
        "import", "from", "const", "let", "var", "pub", "async", "await", "true",
        "false", "null",
    ],
    line_comments: &["//"],
    block_comment: Some(("/*", "*/")),
    string_quotes: &['"', '\''],
    raw_string: None,
    triple_quote: false,
    template_literal: false,
    type_starts_upper: true,
    lifetime_prefix: false,
    attribute_prefix: None,
};

/// Select the appropriate language profile from a language hint string.
pub fn select_profile(language: Option<&str>) -> &'static LangProfile {
    match language.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("rust" | "rs") => &RUST_PROFILE,
        Some("python" | "py") => &PYTHON_PROFILE,
        Some("javascript" | "js" | "typescript" | "ts" | "jsx" | "tsx") => &JS_PROFILE,
        Some("go" | "golang") => &GO_PROFILE,
        Some("java" | "c" | "cpp" | "c++" | "csharp" | "cs") => &JAVA_C_PROFILE,
        Some("bash" | "sh" | "shell" | "zsh") => &SHELL_PROFILE,
        Some("json") => &JSON_PROFILE,
        Some("yaml" | "yml" | "toml") => &YAML_TOML_PROFILE,
        _ => &GENERIC_PROFILE,
    }
}
```

- [ ] **Step 4: Add module declarations**

Add to `src/tui/mod.rs`:
```rust
pub mod lang_profiles;
```

Add to `tests/tui/mod.rs`:
```rust
mod lang_profiles_test;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test lang_profiles_test -v`
Expected: all tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/tui/lang_profiles.rs src/tui/mod.rs tests/tui/lang_profiles_test.rs tests/tui/mod.rs
git commit -m "feat(tui): add language profiles for syntax highlighting"
```

---

## Task 6: CodeBlock Widget

**Files:**
- Create: `src/tui/code_block.rs`
- Create: `tests/tui/code_block_test.rs`
- Modify: `src/tui/mod.rs`
- Modify: `tests/tui/mod.rs`

- [ ] **Step 1: Write failing tests**

```rust
// tests/tui/code_block_test.rs
use viv::core::terminal::buffer::{Buffer, Rect};
use viv::tui::code_block::CodeBlockWidget;
use viv::tui::widget::Widget;

#[test]
fn code_block_renders_border() {
    let widget = CodeBlockWidget::new("fn main() {}", Some("rust"));
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    // Top-left corner should be rounded border
    let ch = buf.get(0, 0).ch;
    assert_eq!(ch, '╭');
}

#[test]
fn code_block_renders_language_label() {
    let widget = CodeBlockWidget::new("code", Some("rust"));
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    // Language label should appear on top border
    let row: String = (0..area.width).map(|x| buf.get(x, 0).ch).collect();
    assert!(row.contains("rust"), "top border should contain language label");
}

#[test]
fn code_block_renders_code_content() {
    let widget = CodeBlockWidget::new("hello", None);
    let area = Rect::new(0, 0, 20, 4);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    // Code should appear on line 1 (inside border)
    let row: String = (0..area.width).map(|x| buf.get(x, 1).ch).collect();
    assert!(row.contains("hello"));
}

#[test]
fn code_block_height_calculation() {
    let code = "line1\nline2\nline3";
    let height = CodeBlockWidget::height(code, 40);
    // 3 code lines + 2 border lines = 5
    assert_eq!(height, 5);
}

#[test]
fn code_block_keyword_gets_color() {
    let widget = CodeBlockWidget::new("fn main", Some("rust"));
    let area = Rect::new(0, 0, 30, 3);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    // 'f' of 'fn' should have a foreground color (keyword color)
    let cell = buf.get(2, 1); // x=2 (after border+space), y=1 (first content row)
    assert!(cell.fg.is_some(), "keyword 'fn' should have syntax color");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test code_block_test 2>&1 | head -10`
Expected: module not found

- [ ] **Step 3: Implement CodeBlockWidget**

```rust
// src/tui/code_block.rs
use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::style::Color;
use crate::tui::block::{Block, BorderStyle};
use crate::tui::syntax::{tokenize, TokenKind};
use crate::tui::widget::Widget;

/// Color theme for syntax tokens.
fn token_color(kind: TokenKind) -> Option<(Color, bool)> {
    match kind {
        TokenKind::Keyword => Some((Color::Rgb(110, 150, 255), true)),
        TokenKind::String => Some((Color::Rgb(120, 200, 120), false)),
        TokenKind::Comment => Some((Color::Rgb(100, 100, 100), false)),
        TokenKind::Number => Some((Color::Rgb(215, 160, 87), false)),
        TokenKind::Type => Some((Color::Rgb(100, 200, 200), false)),
        TokenKind::Function => Some((Color::Rgb(230, 220, 110), false)),
        TokenKind::Operator => Some((Color::Rgb(200, 200, 200), false)),
        TokenKind::Punctuation => Some((Color::Rgb(150, 150, 150), false)),
        TokenKind::Attribute => Some((Color::Rgb(180, 130, 230), false)),
        TokenKind::Lifetime => Some((Color::Rgb(200, 150, 100), false)),
        TokenKind::Plain => None,
    }
}

pub struct CodeBlockWidget<'a> {
    code: &'a str,
    language: Option<&'a str>,
}

impl<'a> CodeBlockWidget<'a> {
    pub fn new(code: &'a str, language: Option<&'a str>) -> Self {
        Self { code, language }
    }

    /// Calculate the rendered height for given code and width.
    pub fn height(code: &str, _width: u16) -> u16 {
        let line_count = if code.is_empty() { 1 } else { code.split('\n').count() };
        (line_count as u16) + 2 // +2 for top/bottom border
    }
}

impl Widget for CodeBlockWidget<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 || area.height < 3 {
            return;
        }

        // Render border
        let mut block = Block::new()
            .border(BorderStyle::Rounded)
            .border_fg(Color::Rgb(80, 80, 80));
        if let Some(lang) = self.language {
            block = block.title(format!(" {} ", lang))
                .title_fg(Color::Rgb(150, 150, 150));
        }
        block.render(area, buf);

        // Render code lines inside the border
        let inner = area.inner();
        for (row_idx, line) in self.code.split('\n').enumerate() {
            if row_idx as u16 >= inner.height {
                break;
            }
            let y = inner.y + row_idx as u16;
            let tokens = tokenize(line, self.language);
            let mut x = inner.x;
            for token in &tokens {
                let (fg, bold) = token_color(token.kind).unwrap_or((Color::Rgb(220, 220, 220), false));
                for ch in token.text.chars() {
                    if x >= inner.x + inner.width {
                        break;
                    }
                    let cell = buf.get_mut(x, y);
                    cell.ch = ch;
                    cell.fg = Some(fg);
                    cell.bold = bold;
                    x += 1;
                }
            }
        }
    }
}
```

- [ ] **Step 4: Add module declarations**

Add to `src/tui/mod.rs`:
```rust
pub mod code_block;
```

Add to `tests/tui/mod.rs`:
```rust
mod code_block_test;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test code_block_test -v`
Expected: all tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/tui/code_block.rs src/tui/mod.rs tests/tui/code_block_test.rs tests/tui/mod.rs
git commit -m "feat(tui): add CodeBlockWidget with syntax highlighting"
```

---

## Task 7: Rewrite Markdown Widget

**Files:**
- Modify: `src/tui/markdown.rs`
- Modify: `tests/tui/markdown_test.rs`

- [ ] **Step 1: Write new tests for MarkdownBlock widget**

Replace `tests/tui/markdown_test.rs`:

```rust
// tests/tui/markdown_test.rs
use viv::core::terminal::buffer::{Buffer, Rect};
use viv::tui::content::{parse_markdown, MarkdownNode, InlineSpan};
use viv::tui::markdown::MarkdownBlockWidget;
use viv::tui::widget::Widget;

#[test]
fn renders_heading_bold() {
    let nodes = parse_markdown("# Hello");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    // 'H' should be bold
    let cell = buf.get(0, 0);
    assert!(cell.bold, "heading text should be bold");
}

#[test]
fn renders_bullet_list() {
    let nodes = parse_markdown("- item one\n- item two");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    let row: String = (0..area.width).map(|x| buf.get(x, 0).ch).collect();
    assert!(row.contains('•'), "unordered list should have bullet");
}

#[test]
fn renders_inline_code_with_color() {
    let nodes = parse_markdown("use `cargo`");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 3);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    // Find the 'c' of 'cargo' — should have Claude orange color
    let mut found = false;
    for x in 0..area.width {
        let cell = buf.get(x, 0);
        if cell.ch == 'c' && cell.fg.is_some() {
            found = true;
            break;
        }
    }
    assert!(found, "inline code should have a highlight color");
}

#[test]
fn renders_quote_with_bar() {
    let nodes = parse_markdown("> quoted");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 40, 3);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    let ch = buf.get(0, 0).ch;
    assert_eq!(ch, '│', "quote should have vertical bar prefix");
}

#[test]
fn renders_horizontal_rule() {
    let nodes = parse_markdown("---");
    let widget = MarkdownBlockWidget::new(&nodes);
    let area = Rect::new(0, 0, 20, 3);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    let ch = buf.get(0, 0).ch;
    assert_eq!(ch, '─', "horizontal rule should render as line");
}

#[test]
fn height_calculation() {
    let nodes = parse_markdown("# Title\n\nParagraph text.\n\n- a\n- b");
    let height = MarkdownBlockWidget::height(&nodes, 40);
    // heading(1+1 blank) + paragraph(1) + list(2) = at least 5
    assert!(height >= 5);
}

// Backward compatibility: render_markdown still works
#[test]
fn render_markdown_compat() {
    let lines = viv::tui::markdown::render_markdown("hello **world**");
    assert!(!lines.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test markdown_test 2>&1 | head -10`
Expected: `MarkdownBlockWidget` not found

- [ ] **Step 3: Rewrite markdown.rs with MarkdownBlockWidget**

Rewrite `src/tui/markdown.rs`:

```rust
// src/tui/markdown.rs
use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::style::Color;
use crate::core::terminal::style::theme;
use crate::tui::code_block::CodeBlockWidget;
use crate::tui::content::{parse_inline, parse_markdown as parse_md, InlineSpan, MarkdownNode};
use crate::tui::paragraph::{Line, Span};
use crate::tui::widget::Widget;

const LINK_COLOR: Color = Color::Rgb(100, 150, 255);
const QUOTE_BAR_COLOR: Color = Color::Rgb(100, 100, 100);

/// Widget that renders a list of MarkdownNodes into a Buffer.
pub struct MarkdownBlockWidget<'a> {
    nodes: &'a [MarkdownNode],
}

impl<'a> MarkdownBlockWidget<'a> {
    pub fn new(nodes: &'a [MarkdownNode]) -> Self {
        Self { nodes }
    }

    /// Calculate rendered height for a given width.
    pub fn height(nodes: &[MarkdownNode], width: u16) -> u16 {
        let mut h: u16 = 0;
        for node in nodes {
            h += node_height(node, width);
        }
        h
    }
}

impl Widget for MarkdownBlockWidget<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let mut y = area.y;
        for node in self.nodes {
            if y >= area.y + area.height {
                break;
            }
            let remaining = Rect::new(area.x, y, area.width, area.y + area.height - y);
            let used = render_node(node, remaining, buf);
            y += used;
        }
    }
}

fn render_node(node: &MarkdownNode, area: Rect, buf: &mut Buffer) -> u16 {
    match node {
        MarkdownNode::Heading { level, text } => {
            render_spans(text, area, buf, true, None);
            1
        }
        MarkdownNode::Paragraph { spans } => {
            render_spans(spans, area, buf, false, None);
            1
        }
        MarkdownNode::List { ordered, items } => {
            let mut y_off = 0u16;
            for (i, item) in items.iter().enumerate() {
                if y_off >= area.height {
                    break;
                }
                let prefix = if *ordered {
                    format!("  {}. ", i + 1)
                } else {
                    "  • ".to_string()
                };
                let sub = Rect::new(area.x, area.y + y_off, area.width, area.height - y_off);
                // Write prefix
                buf.set_str(sub.x, sub.y, &prefix, Some(theme::DIM), false);
                // Write item spans after prefix
                let offset_area = Rect::new(
                    sub.x + prefix.len() as u16,
                    sub.y,
                    sub.width.saturating_sub(prefix.len() as u16),
                    1,
                );
                render_spans(item, offset_area, buf, false, None);
                y_off += 1;
            }
            y_off
        }
        MarkdownNode::Quote { spans } => {
            // Render "│ " prefix in dim
            buf.set_str(area.x, area.y, "│ ", Some(QUOTE_BAR_COLOR), false);
            let inner = Rect::new(area.x + 2, area.y, area.width.saturating_sub(2), 1);
            render_spans(spans, inner, buf, false, Some(theme::DIM));
            1
        }
        MarkdownNode::CodeBlock { language, code } => {
            let widget = CodeBlockWidget::new(code, language.as_deref());
            let h = CodeBlockWidget::height(code, area.width);
            let code_area = Rect::new(area.x, area.y, area.width, h.min(area.height));
            widget.render(code_area, buf);
            h
        }
        MarkdownNode::HorizontalRule => {
            for x in area.x..area.x + area.width {
                buf.set_str(x, area.y, "─", Some(theme::DIM), false);
            }
            1
        }
    }
}

fn render_spans(spans: &[InlineSpan], area: Rect, buf: &mut Buffer, bold_all: bool, fg_override: Option<Color>) {
    let mut x = area.x;
    for span in spans {
        let (text, fg, bold) = match span {
            InlineSpan::Text(t) => (t.as_str(), fg_override.unwrap_or(theme::TEXT), bold_all),
            InlineSpan::Bold(t) => (t.as_str(), fg_override.unwrap_or(theme::TEXT), true),
            InlineSpan::Italic(t) => (t.as_str(), fg_override.unwrap_or(theme::DIM), false),
            InlineSpan::Code(t) => (t.as_str(), fg_override.unwrap_or(theme::CLAUDE), false),
            InlineSpan::Link { text, .. } => (text.as_str(), fg_override.unwrap_or(LINK_COLOR), false),
        };
        for ch in text.chars() {
            if x >= area.x + area.width {
                break;
            }
            let cell = buf.get_mut(x, area.y);
            cell.ch = ch;
            cell.fg = Some(fg);
            cell.bold = bold;
            x += 1;
        }
    }
}

fn node_height(node: &MarkdownNode, width: u16) -> u16 {
    match node {
        MarkdownNode::Heading { .. } => 1,
        MarkdownNode::Paragraph { .. } => 1,
        MarkdownNode::List { items, .. } => items.len() as u16,
        MarkdownNode::Quote { .. } => 1,
        MarkdownNode::CodeBlock { code, .. } => CodeBlockWidget::height(code, width),
        MarkdownNode::HorizontalRule => 1,
    }
}

/// Backward-compatible function: renders Markdown text to Vec<Line> (used by message_style.rs).
pub fn render_markdown(text: &str) -> Vec<Line> {
    let nodes = parse_md(text);
    let mut lines = Vec::new();
    for node in &nodes {
        match node {
            MarkdownNode::Heading { text, .. } => {
                lines.push(Line::from_spans(spans_to_paragraph_spans(text, true)));
            }
            MarkdownNode::Paragraph { spans } => {
                lines.push(Line::from_spans(spans_to_paragraph_spans(spans, false)));
            }
            MarkdownNode::List { ordered, items } => {
                for (i, item) in items.iter().enumerate() {
                    let prefix = if *ordered {
                        format!("  {}. ", i + 1)
                    } else {
                        "  \u{2022} ".to_string()
                    };
                    let mut s = vec![Span::raw(prefix)];
                    s.extend(spans_to_paragraph_spans(item, false));
                    lines.push(Line::from_spans(s));
                }
            }
            MarkdownNode::Quote { spans } => {
                let mut s = vec![Span::styled("│ ", theme::DIM, false)];
                s.extend(spans_to_paragraph_spans(spans, false));
                lines.push(Line::from_spans(s));
            }
            MarkdownNode::CodeBlock { code, .. } => {
                for line in code.split('\n') {
                    lines.push(Line::from_spans(vec![Span::raw(line)]));
                }
            }
            MarkdownNode::HorizontalRule => {
                lines.push(Line::from_spans(vec![Span::styled("────────", theme::DIM, false)]));
            }
        }
    }
    if lines.is_empty() {
        lines.push(Line::from_spans(vec![Span::raw("")]));
    }
    lines
}

fn spans_to_paragraph_spans(spans: &[InlineSpan], bold_all: bool) -> Vec<Span> {
    spans.iter().map(|s| match s {
        InlineSpan::Text(t) => if bold_all { Span::styled(t, theme::TEXT, true) } else { Span::raw(t) },
        InlineSpan::Bold(t) => Span::styled(t, theme::TEXT, true),
        InlineSpan::Italic(t) => Span::styled(t, theme::DIM, false),
        InlineSpan::Code(t) => Span::styled(t, theme::CLAUDE, false),
        InlineSpan::Link { text, .. } => Span::styled(text, LINK_COLOR, false),
    }).collect()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test markdown_test -v`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/tui/markdown.rs tests/tui/markdown_test.rs
git commit -m "feat(tui): rewrite Markdown as MarkdownBlockWidget with CodeBlock integration"
```

---

## Task 8: ToolCall Widget

**Files:**
- Create: `src/tui/tool_call.rs`
- Create: `tests/tui/tool_call_test.rs`
- Modify: `src/tui/mod.rs`
- Modify: `tests/tui/mod.rs`

- [ ] **Step 1: Write failing tests**

```rust
// tests/tui/tool_call_test.rs
use viv::core::terminal::buffer::{Buffer, Rect};
use viv::tui::tool_call::{ToolCallWidget, ToolCallState, ToolStatus, extract_input_summary};
use viv::tui::widget::StatefulWidget;

#[test]
fn folded_renders_single_line() {
    let widget = ToolCallWidget::new("Read", "src/main.rs", &"{\"file_path\": \"src/main.rs\"}");
    let mut state = ToolCallState::new_success("35 lines".into());
    let area = Rect::new(0, 0, 60, 1);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf, &mut state);

    let row: String = (0..area.width).map(|x| buf.get(x, 0).ch).collect();
    assert!(row.contains("Read"), "folded view should show tool name");
    assert!(row.contains('✓'), "success should show checkmark");
}

#[test]
fn folded_height_is_one() {
    let state = ToolCallState::new_success("ok".into());
    assert_eq!(ToolCallWidget::height(&state, 60), 1);
}

#[test]
fn expanded_height_is_more_than_one() {
    let mut state = ToolCallState::new_success("ok".into());
    state.folded = false;
    // Height depends on input/output content, but always > 1
    assert!(ToolCallWidget::height_with_content(&state, "input", Some("output\nline2"), 60) > 1);
}

#[test]
fn error_shows_cross() {
    let widget = ToolCallWidget::new("Edit", "err", "{}");
    let mut state = ToolCallState::new_error("not unique".into());
    let area = Rect::new(0, 0, 60, 1);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf, &mut state);

    let row: String = (0..area.width).map(|x| buf.get(x, 0).ch).collect();
    assert!(row.contains('✗'), "error should show cross");
}

#[test]
fn extract_summary_read() {
    let summary = extract_input_summary("Read", "{\"file_path\": \"/data/main.rs\"}");
    assert!(summary.contains("main.rs"));
}

#[test]
fn extract_summary_bash() {
    let summary = extract_input_summary("Bash", "{\"command\": \"cargo test --release\"}");
    assert!(summary.contains("cargo test"));
}

#[test]
fn extract_summary_grep() {
    let summary = extract_input_summary("Grep", "{\"pattern\": \"fn main\"}");
    assert!(summary.contains("fn main"));
}

#[test]
fn focus_indicator_shows_bar() {
    let widget = ToolCallWidget::new("Read", "file", "{}").focused(true);
    let mut state = ToolCallState::new_success("ok".into());
    let area = Rect::new(0, 0, 60, 1);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf, &mut state);

    let ch = buf.get(0, 0).ch;
    assert_eq!(ch, '┃', "focused tool call should have bar indicator");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test tool_call_test 2>&1 | head -10`
Expected: module not found

- [ ] **Step 3: Implement ToolCallWidget**

```rust
// src/tui/tool_call.rs
use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::style::Color;
use crate::core::terminal::style::theme;
use crate::tui::block::{Block, BorderStyle};
use crate::tui::widget::StatefulWidget;

const SUCCESS_COLOR: Color = Color::Rgb(78, 186, 101);
const ERROR_COLOR: Color = Color::Rgb(171, 43, 63);
const DIM_COLOR: Color = Color::Rgb(136, 136, 136);
const FOCUS_COLOR: Color = Color::Rgb(177, 185, 249);

#[derive(Debug, Clone)]
pub enum ToolStatus {
    Running,
    Success { summary: String },
    Error { message: String },
}

#[derive(Debug, Clone)]
pub struct ToolCallState {
    pub folded: bool,
    pub status: ToolStatus,
    pub output_scroll: u16,
}

impl ToolCallState {
    pub fn new_running() -> Self {
        Self { folded: true, status: ToolStatus::Running, output_scroll: 0 }
    }

    pub fn new_success(summary: String) -> Self {
        Self { folded: true, status: ToolStatus::Success { summary }, output_scroll: 0 }
    }

    pub fn new_error(message: String) -> Self {
        Self { folded: true, status: ToolStatus::Error { message }, output_scroll: 0 }
    }

    pub fn toggle_fold(&mut self) {
        self.folded = !self.folded;
    }
}

pub struct ToolCallWidget<'a> {
    name: &'a str,
    input_summary: &'a str,
    input_raw: &'a str,
    focused: bool,
}

impl<'a> ToolCallWidget<'a> {
    pub fn new(name: &'a str, input_summary: &'a str, input_raw: &'a str) -> Self {
        Self { name, input_summary, input_raw, focused: false }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Height when folded.
    pub fn height(state: &ToolCallState, _width: u16) -> u16 {
        if state.folded { 1 } else { 1 } // expanded height needs content info
    }

    /// Height with actual content for expanded state.
    pub fn height_with_content(state: &ToolCallState, input: &str, output: Option<&str>, _width: u16) -> u16 {
        if state.folded {
            return 1;
        }
        let mut h: u16 = 1; // header line
        // input block: 2 borders + content lines
        let input_lines = input.split('\n').count().max(1) as u16;
        h += input_lines + 2;
        // output block (if present)
        if let Some(out) = output {
            let out_lines = out.split('\n').count().min(20) as u16; // cap at 20
            h += out_lines + 2;
            if out.split('\n').count() > 20 {
                h += 1; // "... (N more lines)" line
            }
        }
        h
    }
}

impl StatefulWidget for ToolCallWidget<'_> {
    type State = ToolCallState;

    fn render(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let mut x = area.x;

        // Focus indicator
        if self.focused {
            buf.set_str(x, area.y, "┃", Some(FOCUS_COLOR), false);
            x += 1;
        } else {
            buf.set_str(x, area.y, " ", None, false);
            x += 1;
        }

        // Gear icon
        buf.set_str(x, area.y, "⚙ ", Some(DIM_COLOR), false);
        x += 3; // ⚙ is 1 wide char + space + space (adjust as needed)

        // Tool name (bold)
        buf.set_str(x, area.y, self.name, Some(theme::TEXT), true);
        x += self.name.len() as u16;
        x += 1; // space

        // Input summary (dim)
        let max_summary = (area.x + area.width).saturating_sub(x + 15) as usize;
        let summary = if self.input_summary.len() > max_summary {
            &self.input_summary[..max_summary]
        } else {
            self.input_summary
        };
        buf.set_str(x, area.y, summary, Some(DIM_COLOR), false);

        // Status (right-aligned)
        let (status_text, status_color) = match &state.status {
            ToolStatus::Running => ("⚙ running...".to_string(), DIM_COLOR),
            ToolStatus::Success { summary } => (format!("✓ {}", summary), SUCCESS_COLOR),
            ToolStatus::Error { message } => (format!("✗ {}", message), ERROR_COLOR),
        };
        let status_x = (area.x + area.width).saturating_sub(status_text.len() as u16 + 1);
        buf.set_str(status_x, area.y, &status_text, Some(status_color), false);

        // Expanded content (if not folded and area has room)
        if !state.folded && area.height > 1 {
            // Input block
            let input_area = Rect::new(
                area.x + 1,
                area.y + 1,
                area.width.saturating_sub(2),
                (area.height - 1).min(4),
            );
            let input_block = Block::new()
                .title(" input ")
                .border(BorderStyle::Rounded)
                .border_fg(Color::Rgb(80, 80, 80));
            input_block.render(input_area, buf);
            let inner = input_area.inner();
            buf.set_str(inner.x, inner.y, self.input_raw, Some(DIM_COLOR), false);
        }
    }
}

/// Extract a human-readable summary from tool input JSON.
pub fn extract_input_summary(tool_name: &str, input_json: &str) -> String {
    let extract_field = |field: &str| -> Option<String> {
        let pattern = format!("\"{}\": \"", field);
        let start = input_json.find(&pattern)?;
        let value_start = start + pattern.len();
        let end = input_json[value_start..].find('"')?;
        Some(input_json[value_start..value_start + end].to_string())
    };

    match tool_name {
        "Read" | "Write" | "Edit" | "MultiEdit" => {
            extract_field("file_path").unwrap_or_default()
        }
        "Bash" => {
            let cmd = extract_field("command").unwrap_or_default();
            if cmd.len() > 60 { format!("{}...", &cmd[..57]) } else { cmd }
        }
        "Grep" => extract_field("pattern").unwrap_or_default(),
        "Glob" => extract_field("pattern").unwrap_or_default(),
        "WebFetch" => extract_field("url").unwrap_or_default(),
        "Agent" | "SubAgent" => {
            let desc = extract_field("description").unwrap_or_default();
            if desc.len() > 40 { format!("{}...", &desc[..37]) } else { desc }
        }
        _ => {
            // First field value
            if let Some(start) = input_json.find("\": \"") {
                let s = start + 4;
                if let Some(end) = input_json[s..].find('"') {
                    let val = &input_json[s..s + end];
                    if val.len() > 50 { return format!("{}...", &val[..47]); }
                    return val.to_string();
                }
            }
            String::new()
        }
    }
}
```

- [ ] **Step 4: Add module declarations**

Add to `src/tui/mod.rs`:
```rust
pub mod tool_call;
```

Add to `tests/tui/mod.rs`:
```rust
mod tool_call_test;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test tool_call_test -v`
Expected: all tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/tui/tool_call.rs src/tui/mod.rs tests/tui/tool_call_test.rs tests/tui/mod.rs
git commit -m "feat(tui): add foldable ToolCallWidget"
```

---

## Task 9: Focus Manager

**Files:**
- Create: `src/tui/focus.rs`
- Create: `tests/tui/focus_test.rs`
- Modify: `src/tui/mod.rs`
- Modify: `tests/tui/mod.rs`

- [ ] **Step 1: Write failing tests**

```rust
// tests/tui/focus_test.rs
use viv::tui::focus::{FocusManager, UIMode};

#[test]
fn initial_state() {
    let fm = FocusManager::new();
    assert_eq!(fm.mode(), UIMode::Normal);
    assert_eq!(fm.focus_index(), 0);
}

#[test]
fn enter_browse_mode() {
    let mut fm = FocusManager::new();
    fm.enter_browse(3);
    assert_eq!(fm.mode(), UIMode::Browse);
}

#[test]
fn exit_browse_mode() {
    let mut fm = FocusManager::new();
    fm.enter_browse(3);
    fm.exit_browse();
    assert_eq!(fm.mode(), UIMode::Normal);
}

#[test]
fn next_focus_wraps() {
    let mut fm = FocusManager::new();
    fm.enter_browse(3);
    fm.next();
    assert_eq!(fm.focus_index(), 1);
    fm.next();
    assert_eq!(fm.focus_index(), 2);
    fm.next();
    assert_eq!(fm.focus_index(), 0); // wraps
}

#[test]
fn prev_focus_wraps() {
    let mut fm = FocusManager::new();
    fm.enter_browse(3);
    fm.prev();
    assert_eq!(fm.focus_index(), 2); // wraps to last
}

#[test]
fn zero_focusable_stays_at_zero() {
    let mut fm = FocusManager::new();
    fm.enter_browse(0);
    fm.next();
    assert_eq!(fm.focus_index(), 0);
}

#[test]
fn is_focused() {
    let mut fm = FocusManager::new();
    fm.enter_browse(3);
    assert!(fm.is_focused(0));
    assert!(!fm.is_focused(1));
    fm.next();
    assert!(fm.is_focused(1));
}

#[test]
fn normal_mode_nothing_focused() {
    let fm = FocusManager::new();
    assert!(!fm.is_focused(0));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test focus_test 2>&1 | head -10`

- [ ] **Step 3: Implement FocusManager**

```rust
// src/tui/focus.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UIMode {
    Normal,
    Browse,
}

#[derive(Debug)]
pub struct FocusManager {
    mode: UIMode,
    focus_index: usize,
    focusable_count: usize,
}

impl FocusManager {
    pub fn new() -> Self {
        Self {
            mode: UIMode::Normal,
            focus_index: 0,
            focusable_count: 0,
        }
    }

    pub fn mode(&self) -> UIMode {
        self.mode
    }

    pub fn focus_index(&self) -> usize {
        self.focus_index
    }

    pub fn enter_browse(&mut self, focusable_count: usize) {
        self.mode = UIMode::Browse;
        self.focusable_count = focusable_count;
        if self.focus_index >= focusable_count && focusable_count > 0 {
            self.focus_index = focusable_count - 1;
        }
    }

    pub fn exit_browse(&mut self) {
        self.mode = UIMode::Normal;
    }

    pub fn next(&mut self) {
        if self.focusable_count == 0 {
            return;
        }
        self.focus_index = (self.focus_index + 1) % self.focusable_count;
    }

    pub fn prev(&mut self) {
        if self.focusable_count == 0 {
            return;
        }
        if self.focus_index == 0 {
            self.focus_index = self.focusable_count - 1;
        } else {
            self.focus_index -= 1;
        }
    }

    /// Returns true if the given tool-call index is currently focused (Browse mode only).
    pub fn is_focused(&self, index: usize) -> bool {
        self.mode == UIMode::Browse && self.focus_index == index
    }

    pub fn update_count(&mut self, count: usize) {
        self.focusable_count = count;
        if self.focus_index >= count && count > 0 {
            self.focus_index = count - 1;
        }
    }
}
```

- [ ] **Step 4: Add module declarations**

Add to `src/tui/mod.rs`:
```rust
pub mod focus;
```

Add to `tests/tui/mod.rs`:
```rust
mod focus_test;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test focus_test -v`
Expected: all tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/tui/focus.rs src/tui/mod.rs tests/tui/focus_test.rs tests/tui/mod.rs
git commit -m "feat(tui): add FocusManager with Browse/Normal modes"
```

---

## Task 10: Conversation Widget with Virtual Scrolling

**Files:**
- Create: `src/tui/conversation.rs`
- Create: `tests/tui/conversation_test.rs`
- Modify: `src/tui/mod.rs`
- Modify: `tests/tui/mod.rs`

- [ ] **Step 1: Write failing tests**

```rust
// tests/tui/conversation_test.rs
use viv::tui::conversation::ConversationState;

#[test]
fn initial_state_auto_follows() {
    let state = ConversationState::new();
    assert!(state.auto_follow);
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn append_height_updates_total() {
    let mut state = ConversationState::new();
    state.append_item_height(3);
    state.append_item_height(5);
    assert_eq!(state.total_height, 8);
    assert_eq!(state.item_heights.len(), 2);
}

#[test]
fn auto_follow_scrolls_to_bottom() {
    let mut state = ConversationState::new();
    state.viewport_height = 10;
    state.append_item_height(5);
    state.append_item_height(5);
    state.append_item_height(5);
    // total=15, viewport=10 → should scroll to 5
    state.auto_scroll();
    assert_eq!(state.scroll_offset, 5);
}

#[test]
fn manual_scroll_disables_auto_follow() {
    let mut state = ConversationState::new();
    state.viewport_height = 10;
    state.append_item_height(20);
    state.auto_scroll();
    state.scroll_up(3);
    assert!(!state.auto_follow);
}

#[test]
fn scroll_to_bottom_restores_auto_follow() {
    let mut state = ConversationState::new();
    state.viewport_height = 10;
    state.append_item_height(20);
    state.auto_scroll();
    state.scroll_up(3);
    state.scroll_to_bottom();
    assert!(state.auto_follow);
}

#[test]
fn visible_range_skips_offscreen() {
    let mut state = ConversationState::new();
    state.viewport_height = 10;
    state.append_item_height(5);  // item 0: rows 0-4
    state.append_item_height(5);  // item 1: rows 5-9
    state.append_item_height(5);  // item 2: rows 10-14
    state.scroll_offset = 5;

    let range = state.visible_items();
    // item 1 starts at row 5 (visible), item 2 starts at row 10 (partially visible at offset 5+10=15)
    assert!(range.len() >= 1);
    assert_eq!(range[0].index, 1);
}

#[test]
fn recalculate_on_resize() {
    let mut state = ConversationState::new();
    state.viewport_height = 10;
    state.append_item_height(5);
    state.append_item_height(5);
    // Simulate resize: heights may change
    state.set_item_height(0, 3);
    state.set_item_height(1, 3);
    assert_eq!(state.total_height, 6);
}

#[test]
fn page_down() {
    let mut state = ConversationState::new();
    state.viewport_height = 10;
    state.append_item_height(30);
    state.page_down();
    assert_eq!(state.scroll_offset, 8); // viewport - 2
}

#[test]
fn scroll_does_not_go_negative() {
    let mut state = ConversationState::new();
    state.viewport_height = 10;
    state.append_item_height(5);
    state.scroll_up(100);
    assert_eq!(state.scroll_offset, 0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test conversation_test 2>&1 | head -10`

- [ ] **Step 3: Implement ConversationState and ConversationWidget**

```rust
// src/tui/conversation.rs
use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::style::Color;
use crate::tui::widget::Widget;

const SCROLLBAR_ACTIVE: Color = Color::Rgb(180, 180, 180);
const SCROLLBAR_INACTIVE: Color = Color::Rgb(60, 60, 60);

/// Describes a visible item and how it maps to the viewport.
#[derive(Debug, Clone)]
pub struct VisibleItem {
    pub index: usize,
    /// Row offset within the item where rendering starts (0 = full item visible from top).
    pub clip_top: u16,
    /// How many rows of this item are visible.
    pub visible_rows: u16,
    /// Y position in the viewport where this item starts rendering.
    pub viewport_y: u16,
}

#[derive(Debug)]
pub struct ConversationState {
    pub scroll_offset: u16,
    pub viewport_height: u16,
    pub auto_follow: bool,
    pub item_heights: Vec<u16>,
    pub total_height: u16,
}

impl ConversationState {
    pub fn new() -> Self {
        Self {
            scroll_offset: 0,
            viewport_height: 0,
            auto_follow: true,
            item_heights: Vec::new(),
            total_height: 0,
        }
    }

    pub fn append_item_height(&mut self, height: u16) {
        self.item_heights.push(height);
        self.total_height += height;
    }

    pub fn set_item_height(&mut self, index: usize, height: u16) {
        if index < self.item_heights.len() {
            let old = self.item_heights[index];
            self.item_heights[index] = height;
            self.total_height = self.total_height - old + height;
        }
    }

    pub fn update_last_height(&mut self, height: u16) {
        if let Some(last) = self.item_heights.last_mut() {
            let old = *last;
            *last = height;
            self.total_height = self.total_height - old + height;
        }
    }

    pub fn recalculate_total(&mut self) {
        self.total_height = self.item_heights.iter().sum();
    }

    pub fn auto_scroll(&mut self) {
        if self.auto_follow {
            let max = self.max_scroll();
            self.scroll_offset = max;
        }
    }

    pub fn scroll_up(&mut self, lines: u16) {
        self.auto_follow = false;
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    pub fn scroll_down(&mut self, lines: u16) {
        self.auto_follow = false;
        let max = self.max_scroll();
        self.scroll_offset = (self.scroll_offset + lines).min(max);
    }

    pub fn page_up(&mut self) {
        let page = self.viewport_height.saturating_sub(2);
        self.scroll_up(page);
    }

    pub fn page_down(&mut self) {
        let page = self.viewport_height.saturating_sub(2);
        self.scroll_down(page);
    }

    pub fn scroll_to_top(&mut self) {
        self.auto_follow = false;
        self.scroll_offset = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.auto_follow = true;
        self.scroll_offset = self.max_scroll();
    }

    fn max_scroll(&self) -> u16 {
        self.total_height.saturating_sub(self.viewport_height)
    }

    /// Compute which items are visible in the current viewport.
    pub fn visible_items(&self) -> Vec<VisibleItem> {
        let mut result = Vec::new();
        let mut cumulative_y: u16 = 0;
        let viewport_end = self.scroll_offset + self.viewport_height;

        for (i, &h) in self.item_heights.iter().enumerate() {
            let item_start = cumulative_y;
            let item_end = cumulative_y + h;

            if item_end <= self.scroll_offset {
                cumulative_y = item_end;
                continue;
            }
            if item_start >= viewport_end {
                break;
            }

            let clip_top = if item_start < self.scroll_offset {
                self.scroll_offset - item_start
            } else {
                0
            };

            let visible_start = item_start.max(self.scroll_offset);
            let visible_end = item_end.min(viewport_end);
            let visible_rows = visible_end - visible_start;
            let viewport_y = visible_start - self.scroll_offset;

            result.push(VisibleItem {
                index: i,
                clip_top,
                visible_rows,
                viewport_y,
            });

            cumulative_y = item_end;
        }

        result
    }

    /// Render a scrollbar into the rightmost column of the area.
    pub fn render_scrollbar(&self, area: Rect, buf: &mut Buffer) {
        if self.total_height <= self.viewport_height || area.height == 0 {
            return;
        }
        let x = area.x + area.width - 1;
        let bar_height = area.height as u32;
        let thumb_size = ((self.viewport_height as u32) * bar_height / self.total_height as u32).max(1);
        let thumb_pos = (self.scroll_offset as u32) * bar_height / self.total_height as u32;

        for row in 0..area.height {
            let y = area.y + row;
            let in_thumb = (row as u32) >= thumb_pos && (row as u32) < thumb_pos + thumb_size;
            let (ch, color) = if in_thumb {
                ('┃', SCROLLBAR_ACTIVE)
            } else {
                ('│', SCROLLBAR_INACTIVE)
            };
            let cell = buf.get_mut(x, y);
            cell.ch = ch;
            cell.fg = Some(color);
        }
    }
}
```

- [ ] **Step 4: Add module declarations**

Add to `src/tui/mod.rs`:
```rust
pub mod conversation;
```

Add to `tests/tui/mod.rs`:
```rust
mod conversation_test;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test conversation_test -v`
Expected: all tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/tui/conversation.rs src/tui/mod.rs tests/tui/conversation_test.rs tests/tui/mod.rs
git commit -m "feat(tui): add ConversationWidget with virtual scrolling"
```

---

## Task 11: Wire Everything into TerminalUI

**Files:**
- Modify: `src/bus/terminal.rs`

This is the integration task. It replaces the flat `Vec<Line>` rendering with the new Widget-based system.

- [ ] **Step 1: Update TerminalUI struct fields**

In `src/bus/terminal.rs`, replace lines 35-61:

**Remove:**
- `history_lines: Vec<Line>`
- `scroll: u16`
- `response_line_idx: Option<usize>`
- `current_response: String`

**Add:**
```rust
use crate::tui::content::{ContentBlock, MarkdownParseBuffer};
use crate::tui::conversation::ConversationState;
use crate::tui::focus::{FocusManager, UIMode};
use crate::tui::tool_call::{ToolCallState, ToolStatus, extract_input_summary};

// New fields in TerminalUI:
blocks: Vec<ContentBlock>,
parse_buffer: MarkdownParseBuffer,
conversation_state: ConversationState,
tool_states: Vec<ToolCallState>,
focus: FocusManager,
mode: UIMode,
tool_seq: usize,
```

- [ ] **Step 2: Update constructor**

In `TerminalUI::new()`, initialize new fields:

```rust
blocks: Vec::new(),
parse_buffer: MarkdownParseBuffer::new(),
conversation_state: ConversationState::new(),
tool_states: Vec::new(),
focus: FocusManager::new(),
mode: UIMode::Normal,
tool_seq: 0,
```

Remove `history_lines` initialization (the welcome message becomes a ContentBlock or stays in header).

- [ ] **Step 3: Update handle_agent_message**

Replace message handlers to use the new content model:

```rust
// AgentMessage::TextChunk(s)
AgentMessage::TextChunk(s) => {
    let new_blocks = self.parse_buffer.push(&s);
    for block in new_blocks {
        let h = self.block_height(&block);
        self.blocks.push(block);
        self.conversation_state.append_item_height(h);
    }
    self.conversation_state.auto_scroll();
    self.dirty = true;
}

// AgentMessage::ToolStart { name, input }
AgentMessage::ToolStart { name, input } => {
    let id = self.tool_seq;
    self.tool_seq += 1;
    let summary = extract_input_summary(&name, &input);
    self.blocks.push(ContentBlock::ToolCall {
        id, name: name.clone(), input, output: None, error: None,
    });
    self.tool_states.push(ToolCallState::new_running());
    self.conversation_state.append_item_height(1);
    self.conversation_state.auto_scroll();
    self.dirty = true;
}

// AgentMessage::ToolEnd { name, output }
AgentMessage::ToolEnd { name, output } => {
    // Find matching running ToolCall (reverse search)
    let tool_idx = self.tool_states.iter().rposition(|s| matches!(s.status, ToolStatus::Running));
    if let Some(idx) = tool_idx {
        let summary = format!("{} chars", output.len());
        self.tool_states[idx].status = ToolStatus::Success { summary };
        // Update ContentBlock output
        if let Some(ContentBlock::ToolCall { output: ref mut o, .. }) = self.blocks.iter_mut()
            .filter(|b| matches!(b, ContentBlock::ToolCall { .. }))
            .nth(idx)
        {
            *o = Some(output);
        }
    }
    self.dirty = true;
}

// AgentMessage::ToolError { name, error }
AgentMessage::ToolError { name, error } => {
    let tool_idx = self.tool_states.iter().rposition(|s| matches!(s.status, ToolStatus::Running));
    if let Some(idx) = tool_idx {
        self.tool_states[idx].status = ToolStatus::Error { message: error.clone() };
        if let Some(ContentBlock::ToolCall { error: ref mut e, .. }) = self.blocks.iter_mut()
            .filter(|b| matches!(b, ContentBlock::ToolCall { .. }))
            .nth(idx)
        {
            *e = Some(error);
        }
    }
    self.dirty = true;
}

// AgentMessage::Done
AgentMessage::Done => {
    // Flush remaining parse buffer
    let remaining = self.parse_buffer.flush();
    for block in remaining {
        let h = self.block_height(&block);
        self.blocks.push(block);
        self.conversation_state.append_item_height(h);
    }
    self.busy = false;
    self.dirty = true;
}

// AgentMessage::Thinking
AgentMessage::Thinking => {
    self.busy = true;
    self.spinner_start = Some(std::time::Instant::now());
    // Flush any pending user input as UserMessage block
    self.dirty = true;
}
```

And when user submits input:

```rust
// In handle_key, on Enter/submit:
let text = self.editor.content();
self.blocks.push(ContentBlock::UserMessage { text: text.clone() });
self.conversation_state.append_item_height(1);
self.conversation_state.auto_scroll();
self.event_tx.send(AgentEvent::Input(text));
```

- [ ] **Step 4: Update handle_key for Browse mode**

Add Browse mode key handling:

```rust
fn handle_key(&mut self, key: KeyEvent) -> Option<UiAction> {
    // ... existing permission handling ...

    match self.focus.mode() {
        UIMode::Browse => {
            match key {
                KeyEvent::Escape => {
                    self.focus.exit_browse();
                    self.dirty = true;
                }
                KeyEvent::Up | KeyEvent::Char('k') => {
                    self.conversation_state.scroll_up(1);
                    self.dirty = true;
                }
                KeyEvent::Down | KeyEvent::Char('j') => {
                    self.conversation_state.scroll_down(1);
                    self.dirty = true;
                }
                KeyEvent::Char('g') => {
                    self.conversation_state.scroll_to_top();
                    self.dirty = true;
                }
                KeyEvent::Char('G') => {
                    self.conversation_state.scroll_to_bottom();
                    self.dirty = true;
                }
                KeyEvent::Tab => {
                    self.focus.next();
                    self.dirty = true;
                }
                KeyEvent::Enter => {
                    let idx = self.focus.focus_index();
                    if idx < self.tool_states.len() {
                        self.tool_states[idx].toggle_fold();
                        // Recalculate height for this block
                        // ... update conversation_state ...
                        self.dirty = true;
                    }
                }
                _ => {}
            }
            return None;
        }
        UIMode::Normal => {
            match key {
                KeyEvent::Escape if !self.busy => {
                    let tc_count = self.tool_states.len();
                    if tc_count > 0 {
                        self.focus.enter_browse(tc_count);
                        self.dirty = true;
                        return None;
                    }
                }
                _ => {} // fall through to existing editor handling
            }
        }
    }

    // ... existing Normal mode key handling (editor, submit, etc.) ...
}
```

- [ ] **Step 5: Update render_frame to use new widgets**

Replace the rendering logic in `render_frame()`:

```rust
fn render_frame(&mut self) {
    let buf = self.renderer.buffer_mut();
    buf.clear();
    let area = self.renderer.area();
    let input_height = (self.editor.line_count() as u16).max(1);
    let chunks = main_layout(input_height).split(area);

    // Header
    self.header.render(chunks[0], buf);

    // Conversation — render visible blocks
    let conv_area = chunks[1];
    self.conversation_state.viewport_height = conv_area.height;
    let visible = self.conversation_state.visible_items();

    let mut tool_call_visual_idx = 0;
    for vi in &visible {
        let block = &self.blocks[vi.index];
        let block_area = Rect::new(
            conv_area.x,
            conv_area.y + vi.viewport_y,
            conv_area.width.saturating_sub(1), // leave 1 col for scrollbar
            vi.visible_rows,
        );
        self.render_content_block(block, block_area, buf, &mut tool_call_visual_idx);
    }

    // Scrollbar
    self.conversation_state.render_scrollbar(conv_area, buf);

    // Input
    let input_widget = InputWidget::new(
        &self.editor.content(),
        self.editor.cursor_offset(),
        "> ",
    );
    input_widget.render(chunks[2], buf);
    let cursor = input_widget.cursor_position(chunks[2]);

    // Status
    let status = StatusWidget {
        model: self.model_name.clone(),
        input_tokens: self.input_tokens,
        output_tokens: self.output_tokens,
    };
    status.render(chunks[3], buf);

    self.renderer.flush(&mut self.backend, Some(cursor)).ok();
}
```

- [ ] **Step 6: Add helper render_content_block**

```rust
fn render_content_block(&self, block: &ContentBlock, area: Rect, buf: &mut Buffer, tool_idx: &mut usize) {
    match block {
        ContentBlock::UserMessage { text } => {
            buf.set_str(area.x, area.y, "> ", Some(theme::DIM), false);
            buf.set_str(area.x + 2, area.y, text, Some(theme::TEXT), false);
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
            let summary = extract_input_summary(name, input);
            let focused = self.focus.is_focused(*tool_idx);
            let widget = ToolCallWidget::new(name, &summary, input).focused(focused);
            if let Some(state) = self.tool_states.get(*tool_idx) {
                let mut state_clone = state.clone();
                widget.render(area, buf, &mut state_clone);
            }
            *tool_idx += 1;
        }
    }
}
```

- [ ] **Step 7: Add block_height helper**

```rust
fn block_height(&self, block: &ContentBlock) -> u16 {
    let width = self.renderer.area().width;
    match block {
        ContentBlock::UserMessage { .. } => 1,
        ContentBlock::Markdown { nodes } => MarkdownBlockWidget::height(nodes, width),
        ContentBlock::CodeBlock { code, .. } => CodeBlockWidget::height(code, width),
        ContentBlock::ToolCall { .. } => 1, // folded by default
    }
}
```

- [ ] **Step 8: Build and fix compilation errors**

Run: `cargo build 2>&1 | head -50`

Fix any remaining compilation issues (import paths, type mismatches, removed fields).

- [ ] **Step 9: Run all tests**

Run: `cargo test 2>&1 | tail -20`
Expected: all existing tests still pass, new tests pass

- [ ] **Step 10: Commit**

```bash
git add src/bus/terminal.rs
git commit -m "feat(tui): wire Widget-based conversation UI into TerminalUI"
```

---

## Task 12: Manual Integration Test

- [ ] **Step 1: Build release**

Run: `cargo build --release`
Expected: clean build

- [ ] **Step 2: Run viv and test golden path**

Run: `cargo run`

Test manually:
1. Welcome screen renders
2. Type a message → UserMessage block appears
3. Agent responds → Markdown renders with formatting
4. Code blocks show syntax highlighting with borders
5. Tool calls show folded with ⚙ icon
6. Press Esc → enter Browse mode
7. Tab between tool calls → focus indicator moves
8. Enter on tool call → expands/collapses
9. j/k scrolls conversation
10. Esc → back to Normal mode, type another message
11. Ctrl+D → clean exit

- [ ] **Step 3: Fix any visual issues found**

Address rendering bugs, spacing, color issues.

- [ ] **Step 4: Run full test suite**

Run: `cargo test && cargo clippy && cargo fmt --check`
Expected: all pass

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "fix(tui): polish Widget framework integration"
```
