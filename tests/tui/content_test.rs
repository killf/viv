use viv::tui::content::{
    ContentBlock, InlineSpan, MarkdownNode, MarkdownParseBuffer, parse_inline, parse_markdown,
};

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

// ── parse_markdown: headings ──────────────────────────────────────────────────

#[test]
fn parse_markdown_h1() {
    let nodes = parse_markdown("# Hello");
    assert_eq!(nodes.len(), 1);
    match &nodes[0] {
        MarkdownNode::Heading { level, text } => {
            assert_eq!(*level, 1);
            assert_eq!(text, &vec![InlineSpan::Text("Hello".to_string())]);
        }
        other => panic!("expected Heading, got {other:?}"),
    }
}

#[test]
fn parse_markdown_h2() {
    let nodes = parse_markdown("## World");
    assert_eq!(nodes.len(), 1);
    match &nodes[0] {
        MarkdownNode::Heading { level, text } => {
            assert_eq!(*level, 2);
            assert_eq!(text, &vec![InlineSpan::Text("World".to_string())]);
        }
        other => panic!("expected Heading, got {other:?}"),
    }
}

// ── parse_markdown: unordered list ───────────────────────────────────────────

#[test]
fn parse_markdown_unordered_list() {
    let nodes = parse_markdown("- alpha\n- beta\n- gamma");
    assert_eq!(nodes.len(), 1);
    match &nodes[0] {
        MarkdownNode::List { ordered, items } => {
            assert!(!ordered);
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], vec![InlineSpan::Text("alpha".to_string())]);
            assert_eq!(items[1], vec![InlineSpan::Text("beta".to_string())]);
            assert_eq!(items[2], vec![InlineSpan::Text("gamma".to_string())]);
        }
        other => panic!("expected List, got {other:?}"),
    }
}

#[test]
fn parse_markdown_unordered_list_star_prefix() {
    let nodes = parse_markdown("* one\n* two");
    assert_eq!(nodes.len(), 1);
    match &nodes[0] {
        MarkdownNode::List { ordered, items } => {
            assert!(!ordered);
            assert_eq!(items.len(), 2);
            assert_eq!(items[0], vec![InlineSpan::Text("one".to_string())]);
            assert_eq!(items[1], vec![InlineSpan::Text("two".to_string())]);
        }
        other => panic!("expected List, got {other:?}"),
    }
}

// ── parse_markdown: ordered list ─────────────────────────────────────────────

#[test]
fn parse_markdown_ordered_list() {
    let nodes = parse_markdown("1. first\n2. second\n3. third");
    assert_eq!(nodes.len(), 1);
    match &nodes[0] {
        MarkdownNode::List { ordered, items } => {
            assert!(ordered);
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], vec![InlineSpan::Text("first".to_string())]);
            assert_eq!(items[1], vec![InlineSpan::Text("second".to_string())]);
            assert_eq!(items[2], vec![InlineSpan::Text("third".to_string())]);
        }
        other => panic!("expected ordered List, got {other:?}"),
    }
}

// ── parse_markdown: quote ─────────────────────────────────────────────────────

#[test]
fn parse_markdown_quote() {
    let nodes = parse_markdown("> Some quoted text");
    assert_eq!(nodes.len(), 1);
    match &nodes[0] {
        MarkdownNode::Quote { spans } => {
            assert_eq!(
                spans,
                &vec![InlineSpan::Text("Some quoted text".to_string())]
            );
        }
        other => panic!("expected Quote, got {other:?}"),
    }
}

// ── parse_markdown: code block ────────────────────────────────────────────────

#[test]
fn parse_markdown_code_block_with_language() {
    let input = "```rust\nfn main() {}\n```";
    let nodes = parse_markdown(input);
    assert_eq!(nodes.len(), 1);
    match &nodes[0] {
        MarkdownNode::CodeBlock { language, code } => {
            assert_eq!(language, &Some("rust".to_string()));
            assert_eq!(code, "fn main() {}");
        }
        other => panic!("expected CodeBlock, got {other:?}"),
    }
}

#[test]
fn parse_markdown_code_block_without_language() {
    let input = "```\nhello\nworld\n```";
    let nodes = parse_markdown(input);
    assert_eq!(nodes.len(), 1);
    match &nodes[0] {
        MarkdownNode::CodeBlock { language, code } => {
            assert_eq!(language, &None);
            assert_eq!(code, "hello\nworld");
        }
        other => panic!("expected CodeBlock, got {other:?}"),
    }
}

// ── parse_markdown: horizontal rule ──────────────────────────────────────────

#[test]
fn parse_markdown_horizontal_rule_dashes() {
    let nodes = parse_markdown("---");
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0], MarkdownNode::HorizontalRule);
}

#[test]
fn parse_markdown_horizontal_rule_stars() {
    let nodes = parse_markdown("***");
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0], MarkdownNode::HorizontalRule);
}

#[test]
fn parse_markdown_horizontal_rule_underscores() {
    let nodes = parse_markdown("___");
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0], MarkdownNode::HorizontalRule);
}

// ── parse_markdown: paragraph ─────────────────────────────────────────────────

#[test]
fn parse_markdown_paragraph() {
    let nodes = parse_markdown("Just some plain text.");
    assert_eq!(nodes.len(), 1);
    match &nodes[0] {
        MarkdownNode::Paragraph { spans } => {
            assert_eq!(
                spans,
                &vec![InlineSpan::Text("Just some plain text.".to_string())]
            );
        }
        other => panic!("expected Paragraph, got {other:?}"),
    }
}

// ── parse_markdown: mixed blocks ──────────────────────────────────────────────

#[test]
fn parse_markdown_mixed_blocks() {
    let input = "# Title\n\nHello world\n\n- item1\n- item2\n\n```\ncode here\n```";
    let nodes = parse_markdown(input);
    // Expected: Heading, Paragraph, List, CodeBlock
    assert_eq!(nodes.len(), 4, "expected 4 nodes, got {nodes:?}");
    assert!(matches!(nodes[0], MarkdownNode::Heading { level: 1, .. }));
    assert!(matches!(nodes[1], MarkdownNode::Paragraph { .. }));
    assert!(matches!(
        nodes[2],
        MarkdownNode::List { ordered: false, .. }
    ));
    assert!(matches!(nodes[3], MarkdownNode::CodeBlock { .. }));
}

// ── parse_markdown: empty input ───────────────────────────────────────────────

#[test]
fn parse_markdown_empty_input() {
    let nodes = parse_markdown("");
    assert!(nodes.is_empty(), "empty input should return empty vec");
}

// ── MarkdownParseBuffer: streaming tests ──────────────────────────────────────

#[test]
fn stream_buffer_complete_line() {
    let mut buf = MarkdownParseBuffer::new();
    let blocks = buf.push("Hello world\n");
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        ContentBlock::Markdown { nodes } => {
            assert_eq!(nodes.len(), 1);
            assert!(matches!(nodes[0], MarkdownNode::Paragraph { .. }));
        }
        other => panic!("expected Markdown block, got {other:?}"),
    }
}

#[test]
fn stream_buffer_incomplete_line() {
    let mut buf = MarkdownParseBuffer::new();

    // No newline yet — nothing should be emitted
    let blocks = buf.push("Hello");
    assert!(
        blocks.is_empty(),
        "incomplete line must not emit blocks yet"
    );

    // Now complete the line
    let blocks = buf.push(" world\n");
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        ContentBlock::Markdown { nodes } => {
            assert_eq!(nodes.len(), 1);
            match &nodes[0] {
                MarkdownNode::Paragraph { spans } => {
                    let text: String = spans
                        .iter()
                        .filter_map(|s| {
                            if let InlineSpan::Text(t) = s {
                                Some(t.as_str())
                            } else {
                                None
                            }
                        })
                        .collect();
                    assert!(text.contains("Hello world"), "got: {text:?}");
                }
                other => panic!("expected Paragraph, got {other:?}"),
            }
        }
        other => panic!("expected Markdown block, got {other:?}"),
    }
}

#[test]
fn stream_buffer_code_block() {
    let mut buf = MarkdownParseBuffer::new();

    // Open fence
    let b1 = buf.push("```rust\n");
    assert!(b1.is_empty(), "opening fence alone should not emit");

    // Code lines
    let b2 = buf.push("fn foo() {}\n");
    assert!(b2.is_empty(), "code line inside block should not emit");

    // Close fence
    let b3 = buf.push("```\n");
    assert_eq!(b3.len(), 1);
    match &b3[0] {
        ContentBlock::CodeBlock { language, code } => {
            assert_eq!(language, &Some("rust".to_string()));
            assert_eq!(code, "fn foo() {}");
        }
        other => panic!("expected CodeBlock, got {other:?}"),
    }
}

#[test]
fn stream_buffer_flush_pending() {
    let mut buf = MarkdownParseBuffer::new();

    // Push text without trailing newline
    buf.push("Pending text");

    // flush() should emit whatever is buffered
    let blocks = buf.flush();
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        ContentBlock::Markdown { nodes } => {
            assert!(!nodes.is_empty(), "flushed nodes must not be empty");
        }
        other => panic!("expected Markdown block, got {other:?}"),
    }
}

#[test]
fn stream_buffer_code_block_promotes() {
    // Code blocks streamed in via MarkdownParseBuffer must become
    // ContentBlock::CodeBlock, NOT ContentBlock::Markdown.
    let mut buf = MarkdownParseBuffer::new();

    buf.push("```python\n");
    buf.push("print('hi')\n");
    let blocks = buf.push("```\n");

    assert_eq!(blocks.len(), 1);
    assert!(
        matches!(blocks[0], ContentBlock::CodeBlock { .. }),
        "streaming code block must emit ContentBlock::CodeBlock, got {:?}",
        blocks[0]
    );
}
