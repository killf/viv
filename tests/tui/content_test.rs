use viv::tui::content::{parse_inline, InlineSpan};

// ── parse_plain_text ──────────────────────────────────────────────────────────

#[test]
fn parse_plain_text() {
    let spans = parse_inline("hello world");
    assert_eq!(spans, vec![InlineSpan::Text("hello world".to_string())]);
}

// ── parse_bold ────────────────────────────────────────────────────────────────

#[test]
fn parse_bold() {
    let spans = parse_inline("**bold**");
    assert_eq!(spans, vec![InlineSpan::Bold("bold".to_string())]);
}

#[test]
fn parse_bold_with_surrounding_text() {
    let spans = parse_inline("before **bold** after");
    assert_eq!(
        spans,
        vec![
            InlineSpan::Text("before ".to_string()),
            InlineSpan::Bold("bold".to_string()),
            InlineSpan::Text(" after".to_string()),
        ]
    );
}

// ── parse_italic ──────────────────────────────────────────────────────────────

#[test]
fn parse_italic() {
    let spans = parse_inline("*italic*");
    assert_eq!(spans, vec![InlineSpan::Italic("italic".to_string())]);
}

#[test]
fn parse_italic_with_surrounding_text() {
    let spans = parse_inline("before *italic* after");
    assert_eq!(
        spans,
        vec![
            InlineSpan::Text("before ".to_string()),
            InlineSpan::Italic("italic".to_string()),
            InlineSpan::Text(" after".to_string()),
        ]
    );
}

// ── parse_inline_code ─────────────────────────────────────────────────────────

#[test]
fn parse_inline_code() {
    let spans = parse_inline("`code`");
    assert_eq!(spans, vec![InlineSpan::Code("code".to_string())]);
}

#[test]
fn parse_inline_code_with_surrounding_text() {
    let spans = parse_inline("run `cargo test` now");
    assert_eq!(
        spans,
        vec![
            InlineSpan::Text("run ".to_string()),
            InlineSpan::Code("cargo test".to_string()),
            InlineSpan::Text(" now".to_string()),
        ]
    );
}

// ── parse_link ────────────────────────────────────────────────────────────────

#[test]
fn parse_link() {
    let spans = parse_inline("[text](https://example.com)");
    assert_eq!(
        spans,
        vec![InlineSpan::Link {
            text: "text".to_string(),
            url: "https://example.com".to_string(),
        }]
    );
}

#[test]
fn parse_link_with_surrounding_text() {
    let spans = parse_inline("see [docs](https://docs.rs) for details");
    assert_eq!(
        spans,
        vec![
            InlineSpan::Text("see ".to_string()),
            InlineSpan::Link {
                text: "docs".to_string(),
                url: "https://docs.rs".to_string(),
            },
            InlineSpan::Text(" for details".to_string()),
        ]
    );
}

// ── parse_mixed_inline ────────────────────────────────────────────────────────

#[test]
fn parse_mixed_inline_bold_and_code() {
    let spans = parse_inline("**bold** and `code`");
    assert_eq!(
        spans,
        vec![
            InlineSpan::Bold("bold".to_string()),
            InlineSpan::Text(" and ".to_string()),
            InlineSpan::Code("code".to_string()),
        ]
    );
}

#[test]
fn parse_mixed_inline_italic_link_code() {
    let spans = parse_inline("*hello* [click](http://x.com) `y`");
    assert_eq!(
        spans,
        vec![
            InlineSpan::Italic("hello".to_string()),
            InlineSpan::Text(" ".to_string()),
            InlineSpan::Link {
                text: "click".to_string(),
                url: "http://x.com".to_string(),
            },
            InlineSpan::Text(" ".to_string()),
            InlineSpan::Code("y".to_string()),
        ]
    );
}

// ── parse_unclosed_bold ───────────────────────────────────────────────────────

#[test]
fn parse_unclosed_bold_content_not_dropped() {
    let spans = parse_inline("see **code");
    let all_text: String = spans
        .iter()
        .map(|s| match s {
            InlineSpan::Text(t) => t.as_str(),
            InlineSpan::Bold(t) => t.as_str(),
            InlineSpan::Italic(t) => t.as_str(),
            InlineSpan::Code(t) => t.as_str(),
            InlineSpan::Link { text, .. } => text.as_str(),
        })
        .collect();
    assert!(
        all_text.contains("code"),
        "content after unclosed ** must not be dropped; got: {all_text:?}"
    );
}

#[test]
fn parse_unclosed_italic_content_not_dropped() {
    let spans = parse_inline("text *unclosed");
    let all_text: String = spans
        .iter()
        .map(|s| match s {
            InlineSpan::Text(t) => t.as_str(),
            InlineSpan::Bold(t) => t.as_str(),
            InlineSpan::Italic(t) => t.as_str(),
            InlineSpan::Code(t) => t.as_str(),
            InlineSpan::Link { text, .. } => text.as_str(),
        })
        .collect();
    assert!(
        all_text.contains("unclosed"),
        "content after unclosed * must not be dropped; got: {all_text:?}"
    );
}

#[test]
fn parse_unclosed_code_content_not_dropped() {
    let spans = parse_inline("text `unclosed");
    let all_text: String = spans
        .iter()
        .map(|s| match s {
            InlineSpan::Text(t) => t.as_str(),
            InlineSpan::Bold(t) => t.as_str(),
            InlineSpan::Italic(t) => t.as_str(),
            InlineSpan::Code(t) => t.as_str(),
            InlineSpan::Link { text, .. } => text.as_str(),
        })
        .collect();
    assert!(
        all_text.contains("unclosed"),
        "content after unclosed ` must not be dropped; got: {all_text:?}"
    );
}

// ── parse_empty ───────────────────────────────────────────────────────────────

#[test]
fn parse_empty() {
    let spans = parse_inline("");
    assert_eq!(spans, vec![InlineSpan::Text(String::new())]);
}
