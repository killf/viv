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
    chars[start..].iter().position(|&c| c == '`').map(|p| p + start)
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
