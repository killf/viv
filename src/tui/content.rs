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
    Heading {
        level: u8,
        text: Vec<InlineSpan>,
    },
    Paragraph {
        spans: Vec<InlineSpan>,
    },
    List {
        ordered: bool,
        items: Vec<Vec<InlineSpan>>,
    },
    Quote {
        spans: Vec<InlineSpan>,
    },
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    HorizontalRule,
}

/// Top-level content blocks in a conversation.
#[derive(Debug, Clone)]
pub enum ContentBlock {
    UserMessage {
        text: String,
    },
    Markdown {
        nodes: Vec<MarkdownNode>,
    },
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    ToolCall {
        id: usize,
        name: String,
        input: String,
        output: Option<String>,
        error: Option<String>,
    },
}

/// Parse inline Markdown in `line` into a sequence of [`InlineSpan`]s.
///
/// Recognised markers (all single-pass, left-to-right):
/// - `**text**` → [`InlineSpan::Bold`]
/// - `*text*`   → [`InlineSpan::Italic`]  (single star, not double)
/// - `` `text` `` → [`InlineSpan::Code`]
/// - `[text](url)` → [`InlineSpan::Link`]
/// - Everything else → [`InlineSpan::Text`]
///
/// Unclosed markers are treated as literal text so no content is dropped.
/// An empty input returns `vec![InlineSpan::Text(String::new())]`.
pub fn parse_inline(line: &str) -> Vec<InlineSpan> {
    let mut spans: Vec<InlineSpan> = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut buf = String::new();

    // Helper: flush `buf` into a Text span if non-empty.
    macro_rules! flush_buf {
        () => {
            if !buf.is_empty() {
                spans.push(InlineSpan::Text(buf.clone()));
                buf.clear();
            }
        };
    }

    while i < len {
        // ── **bold** ─────────────────────────────────────────────────────────
        if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
            // Look for closing **.
            if let Some(close) = find_close_double_star(&chars, i + 2) {
                flush_buf!();
                let inner: String = chars[i + 2..close].iter().collect();
                spans.push(InlineSpan::Bold(inner));
                i = close + 2;
            } else {
                // No closing ** found — emit as literal text.
                buf.push('*');
                buf.push('*');
                i += 2;
            }
        // ── *italic* ──────────────────────────────────────────────────────────
        } else if chars[i] == '*' {
            // Look for a closing single *.
            if let Some(close) = find_close_single_star(&chars, i + 1) {
                flush_buf!();
                let inner: String = chars[i + 1..close].iter().collect();
                spans.push(InlineSpan::Italic(inner));
                i = close + 1;
            } else {
                buf.push('*');
                i += 1;
            }
        // ── `code` ────────────────────────────────────────────────────────────
        } else if chars[i] == '`' {
            if let Some(close) = find_close_backtick(&chars, i + 1) {
                flush_buf!();
                let inner: String = chars[i + 1..close].iter().collect();
                spans.push(InlineSpan::Code(inner));
                i = close + 1;
            } else {
                buf.push('`');
                i += 1;
            }
        // ── [text](url) ───────────────────────────────────────────────────────
        } else if chars[i] == '[' {
            if let Some((text, url, end)) = try_parse_link(&chars, i) {
                flush_buf!();
                spans.push(InlineSpan::Link { text, url });
                i = end;
            } else {
                buf.push('[');
                i += 1;
            }
        } else {
            buf.push(chars[i]);
            i += 1;
        }
    }

    flush_buf!();

    if spans.is_empty() {
        spans.push(InlineSpan::Text(String::new()));
    }

    spans
}

// ── internal helpers ──────────────────────────────────────────────────────────

/// Find the index of the first `**` at or after `start` (returns the index of
/// the first `*` of the closing pair).
fn find_close_double_star(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start;
    while i + 1 < chars.len() {
        if chars[i] == '*' && chars[i + 1] == '*' {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Find the index of the first single `*` (not followed by another `*`, and
/// not preceded by another `*`) at or after `start`.
fn find_close_single_star(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start;
    while i < chars.len() {
        if chars[i] == '*' {
            // Make sure it's not part of a ** pair.
            let prev_star = i > 0 && chars[i - 1] == '*';
            let next_star = i + 1 < chars.len() && chars[i + 1] == '*';
            if !prev_star && !next_star {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Find the index of the first `` ` `` at or after `start`.
fn find_close_backtick(chars: &[char], start: usize) -> Option<usize> {
    chars[start..]
        .iter()
        .position(|&c| c == '`')
        .map(|p| p + start)
}

/// Try to parse `[text](url)` starting at `chars[start]` (`[`).
/// Returns `(text, url, index_after_closing_paren)` on success.
fn try_parse_link(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    // Find closing ]
    let close_bracket = chars[start + 1..]
        .iter()
        .position(|&c| c == ']')
        .map(|p| p + start + 1)?;

    // Must be followed by (
    if close_bracket + 1 >= chars.len() || chars[close_bracket + 1] != '(' {
        return None;
    }

    // Find closing )
    let open_paren = close_bracket + 1;
    let close_paren = chars[open_paren + 1..]
        .iter()
        .position(|&c| c == ')')
        .map(|p| p + open_paren + 1)?;

    let text: String = chars[start + 1..close_bracket].iter().collect();
    let url: String = chars[open_paren + 1..close_paren].iter().collect();

    Some((text, url, close_paren + 1))
}

// ── parse_markdown ────────────────────────────────────────────────────────────

/// Parse full Markdown text into block-level [`MarkdownNode`]s.
///
/// Rules (evaluated in order per line):
/// - Empty lines → skip (paragraph separators)
/// - ` ```lang ` / ` ``` ` → [`MarkdownNode::CodeBlock`]
/// - `---` / `***` / `___` → [`MarkdownNode::HorizontalRule`]
/// - `# ` … `###### ` → [`MarkdownNode::Heading`]
/// - `> text` → [`MarkdownNode::Quote`]
/// - `- text` / `* text` (consecutive) → unordered [`MarkdownNode::List`]
/// - `1. text` (consecutive) → ordered [`MarkdownNode::List`]
/// - Everything else → [`MarkdownNode::Paragraph`]
pub fn parse_markdown(text: &str) -> Vec<MarkdownNode> {
    let mut nodes: Vec<MarkdownNode> = Vec::new();
    let mut lines = text.lines().peekable();

    while let Some(line) = lines.next() {
        // ── code fence open ───────────────────────────────────────────────────
        if line.starts_with("```") {
            let lang_hint = line[3..].trim();
            let language = if lang_hint.is_empty() {
                None
            } else {
                Some(lang_hint.to_string())
            };
            let mut code_lines: Vec<&str> = Vec::new();
            // consume until closing fence or EOF
            loop {
                match lines.next() {
                    Some(l) if l.trim_start().starts_with("```") => break,
                    Some(l) => code_lines.push(l),
                    None => break,
                }
            }
            let code = code_lines.join("\n");
            nodes.push(MarkdownNode::CodeBlock { language, code });
            continue;
        }

        // ── horizontal rule ───────────────────────────────────────────────────
        if line == "---" || line == "***" || line == "___" {
            nodes.push(MarkdownNode::HorizontalRule);
            continue;
        }

        // ── heading ───────────────────────────────────────────────────────────
        if line.starts_with('#') {
            let level = line.chars().take_while(|&c| c == '#').count();
            if level <= 6 {
                let rest = line[level..].trim_start_matches(' ');
                nodes.push(MarkdownNode::Heading {
                    level: level as u8,
                    text: parse_inline(rest),
                });
                continue;
            }
        }

        // ── blockquote ────────────────────────────────────────────────────────
        if let Some(rest) = line.strip_prefix("> ") {
            nodes.push(MarkdownNode::Quote {
                spans: parse_inline(rest),
            });
            continue;
        }
        if line == ">" {
            nodes.push(MarkdownNode::Quote {
                spans: parse_inline(""),
            });
            continue;
        }

        // ── unordered list ────────────────────────────────────────────────────
        if is_unordered_item(line) {
            let mut items: Vec<Vec<InlineSpan>> = Vec::new();
            items.push(parse_inline(unordered_item_text(line)));
            // greedily consume consecutive unordered items
            while lines.peek().map(|l| is_unordered_item(l)).unwrap_or(false) {
                let next = lines.next().unwrap();
                items.push(parse_inline(unordered_item_text(next)));
            }
            nodes.push(MarkdownNode::List {
                ordered: false,
                items,
            });
            continue;
        }

        // ── ordered list ──────────────────────────────────────────────────────
        if is_ordered_item(line) {
            let mut items: Vec<Vec<InlineSpan>> = Vec::new();
            items.push(parse_inline(ordered_item_text(line)));
            while lines.peek().map(|l| is_ordered_item(l)).unwrap_or(false) {
                let next = lines.next().unwrap();
                items.push(parse_inline(ordered_item_text(next)));
            }
            nodes.push(MarkdownNode::List {
                ordered: true,
                items,
            });
            continue;
        }

        // ── empty line (paragraph separator) ─────────────────────────────────
        if line.trim().is_empty() {
            continue;
        }

        // ── paragraph ─────────────────────────────────────────────────────────
        nodes.push(MarkdownNode::Paragraph {
            spans: parse_inline(line),
        });
    }

    nodes
}

fn is_unordered_item(line: &str) -> bool {
    (line.starts_with("- ") || line.starts_with("* ")) && line.len() > 2
}

fn unordered_item_text(line: &str) -> &str {
    &line[2..]
}

fn is_ordered_item(line: &str) -> bool {
    // Match "N. " where N is one or more digits
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    i > 0 && i + 1 < bytes.len() && bytes[i] == b'.' && bytes[i + 1] == b' '
}

fn ordered_item_text(line: &str) -> &str {
    // Skip past "N. "
    let dot = line.find(". ").unwrap();
    &line[dot + 2..]
}

// ── MarkdownParseBuffer ───────────────────────────────────────────────────────

/// A streaming Markdown parser that accepts text chunks and emits complete
/// [`ContentBlock`]s as they become available.
pub struct MarkdownParseBuffer {
    buffer: String,
    in_code_block: bool,
    code_language: Option<String>,
    code_lines: Vec<String>,
}

impl MarkdownParseBuffer {
    pub fn new() -> Self {
        MarkdownParseBuffer {
            buffer: String::new(),
            in_code_block: false,
            code_language: None,
            code_lines: Vec::new(),
        }
    }

    /// Append `chunk` to the internal buffer and return any complete
    /// [`ContentBlock`]s that are now ready.
    pub fn push(&mut self, chunk: &str) -> Vec<ContentBlock> {
        self.buffer.push_str(chunk);
        self.drain_complete_lines()
    }

    /// Flush any remaining buffered content and return blocks.
    pub fn flush(&mut self) -> Vec<ContentBlock> {
        // If there's no trailing newline, add one so the remainder is processed.
        if !self.buffer.is_empty() && !self.buffer.ends_with('\n') {
            self.buffer.push('\n');
        }
        let mut blocks = self.drain_complete_lines();

        // If we're still inside a code block, emit what we have.
        if self.in_code_block {
            let language = self.code_language.take();
            let code = self.code_lines.join("\n");
            self.code_lines.clear();
            self.in_code_block = false;
            blocks.push(ContentBlock::CodeBlock { language, code });
        }

        blocks
    }

    /// Extract all complete lines from the buffer and parse them into blocks.
    fn drain_complete_lines(&mut self) -> Vec<ContentBlock> {
        let mut blocks: Vec<ContentBlock> = Vec::new();
        // Collect pending non-code lines to batch into a single Markdown block.
        let mut markdown_lines: Vec<String> = Vec::new();

        while let Some(pos) = self.buffer.find('\n') {
            let line: String = self.buffer[..pos].to_string();
            self.buffer.drain(..=pos);

            if self.in_code_block {
                // Closing fence?
                if line.trim_start().starts_with("```") {
                    // Flush any pending markdown before the code block
                    if !markdown_lines.is_empty() {
                        let text = markdown_lines.join("\n");
                        markdown_lines.clear();
                        let nodes = parse_markdown(&text);
                        if !nodes.is_empty() {
                            blocks.push(ContentBlock::Markdown { nodes });
                        }
                    }
                    let language = self.code_language.take();
                    let code = self.code_lines.join("\n");
                    self.code_lines.clear();
                    self.in_code_block = false;
                    blocks.push(ContentBlock::CodeBlock { language, code });
                } else {
                    self.code_lines.push(line);
                }
            } else if line.starts_with("```") {
                // Flush any pending markdown lines first.
                if !markdown_lines.is_empty() {
                    let text = markdown_lines.join("\n");
                    markdown_lines.clear();
                    let nodes = parse_markdown(&text);
                    if !nodes.is_empty() {
                        blocks.push(ContentBlock::Markdown { nodes });
                    }
                }
                // Enter code block mode.
                self.in_code_block = true;
                let lang_hint = line[3..].trim();
                self.code_language = if lang_hint.is_empty() {
                    None
                } else {
                    Some(lang_hint.to_string())
                };
            } else {
                // Regular line — accumulate for Markdown parsing.
                markdown_lines.push(line);
            }
        }

        // Emit any accumulated Markdown lines.
        if !markdown_lines.is_empty() {
            let text = markdown_lines.join("\n");
            let nodes = parse_markdown(&text);
            if !nodes.is_empty() {
                blocks.push(ContentBlock::Markdown { nodes });
            }
        }

        blocks
    }
}
