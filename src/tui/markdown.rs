use crate::core::terminal::style::theme;
use crate::tui::paragraph::{Line, Span};

pub fn render_markdown(text: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut in_code_block = false;

    for input_line in text.split('\n') {
        let trimmed = input_line.trim_end();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }

        if in_code_block {
            lines.push(Line::from_spans(vec![Span::raw(trimmed)]));
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("### ") {
            let mut spans = vec![Span::raw("  ")];
            spans.push(Span::styled(rest, theme::TEXT, true));
            lines.push(Line::from_spans(spans));
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            lines.push(Line::from_spans(vec![Span::styled(rest, theme::TEXT, true)]));
        } else if let Some(rest) = trimmed.strip_prefix("# ") {
            lines.push(Line::from_spans(vec![Span::styled(rest, theme::TEXT, true)]));
        } else if let Some(rest) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            let mut spans = vec![Span::raw("  \u{2022} ")];
            spans.extend(parse_inline(rest));
            lines.push(Line::from_spans(spans));
        } else if is_ordered_list(trimmed) {
            let dot_pos = trimmed.find(". ").unwrap();
            let number = &trimmed[..dot_pos];
            let rest = &trimmed[dot_pos + 2..];
            let mut spans = vec![Span::raw("  "), Span::raw(format!("{}. ", number))];
            spans.extend(parse_inline(rest));
            lines.push(Line::from_spans(spans));
        } else {
            lines.push(Line::from_spans(parse_inline(trimmed)));
        }
    }

    if lines.is_empty() {
        lines.push(Line::from_spans(vec![Span::raw("")]));
    }

    lines
}

fn is_ordered_list(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    i > 0 && i < bytes.len() && bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1] == b' '
}

fn parse_inline(line: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut buf = String::new();

    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            if !buf.is_empty() {
                spans.push(Span::raw(buf.clone()));
                buf.clear();
            }
            i += 2;
            let mut inner = String::new();
            while i < chars.len() {
                if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
                    break;
                }
                inner.push(chars[i]);
                i += 1;
            }
            // consume closing ** if present
            if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
                i += 2;
            }
            spans.push(Span::styled(inner, theme::TEXT, true));
        } else if i + 1 < chars.len() && chars[i] == '_' && chars[i + 1] == '_' {
            if !buf.is_empty() {
                spans.push(Span::raw(buf.clone()));
                buf.clear();
            }
            i += 2;
            let mut inner = String::new();
            while i < chars.len() {
                if i + 1 < chars.len() && chars[i] == '_' && chars[i + 1] == '_' {
                    break;
                }
                inner.push(chars[i]);
                i += 1;
            }
            // consume closing __ if present
            if i + 1 < chars.len() && chars[i] == '_' && chars[i + 1] == '_' {
                i += 2;
            }
            spans.push(Span::styled(inner, theme::TEXT, true));
        } else if chars[i] == '`' {
            if !buf.is_empty() {
                spans.push(Span::raw(buf.clone()));
                buf.clear();
            }
            i += 1;
            let mut inner = String::new();
            while i < chars.len() && chars[i] != '`' {
                inner.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                i += 1;
            }
            spans.push(Span::styled(inner, theme::SUGGESTION, false));
        } else {
            buf.push(chars[i]);
            i += 1;
        }
    }

    if !buf.is_empty() {
        spans.push(Span::raw(buf));
    }

    if spans.is_empty() {
        spans.push(Span::raw(""));
    }

    spans
}
