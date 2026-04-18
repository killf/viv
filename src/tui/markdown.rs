use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::style::Color;
use crate::core::terminal::style::theme;
use crate::tui::code_block::CodeBlockWidget;
use crate::tui::content::{
    InlineSpan, MarkdownNode, parse_inline as parse_inline_content, parse_markdown as parse_md,
};
use crate::tui::paragraph::{Line, Span};
use crate::tui::widget::Widget;

pub struct MarkdownBlockWidget<'a> {
    nodes: &'a [MarkdownNode],
}

impl<'a> MarkdownBlockWidget<'a> {
    pub fn new(nodes: &'a [MarkdownNode]) -> Self {
        MarkdownBlockWidget { nodes }
    }

    /// Sum of heights of all nodes given the available width.
    pub fn height(nodes: &[MarkdownNode], width: u16) -> u16 {
        let mut total: u16 = 0;
        for node in nodes {
            total = total.saturating_add(node_height(node, width));
        }
        total
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

fn inline_span_to_span(span: &InlineSpan, bold_context: bool) -> Span {
    match span {
        InlineSpan::Text(s) => Span {
            text: s.clone(),
            fg: Some(theme::TEXT),
            bold: bold_context,
        },
        InlineSpan::Bold(s) => Span::styled(s.clone(), theme::TEXT, true),
        InlineSpan::Italic(s) => Span::styled(s.clone(), theme::DIM, false),
        InlineSpan::Code(s) => Span::styled(s.clone(), Color::Rgb(215, 119, 87), false),
        InlineSpan::Link { text, .. } => {
            Span::styled(text.clone(), Color::Rgb(100, 150, 255), false)
        }
    }
}

fn render_inline_spans(
    spans: &[InlineSpan],
    x: u16,
    y: u16,
    buf: &mut Buffer,
    area: Rect,
    bold_context: bool,
) {
    let mut cur_x = x;
    let max_x = area.x + area.width;
    for inline in spans {
        let sp = inline_span_to_span(inline, bold_context);
        for ch in sp.text.chars() {
            if cur_x >= max_x {
                break;
            }
            let cell = buf.get_mut(cur_x, y);
            cell.ch = ch;
            cell.fg = sp.fg;
            cell.bold = sp.bold;
            cur_x += 1;
        }
        if cur_x >= max_x {
            break;
        }
    }
}

impl<'a> Widget for MarkdownBlockWidget<'a> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let mut row = area.y;

        for node in self.nodes {
            if row >= area.y + area.height {
                break;
            }

            match node {
                MarkdownNode::Heading { text, .. } => {
                    render_inline_spans(text, area.x, row, buf, area, true);
                    row += 1;
                }

                MarkdownNode::Paragraph { spans } => {
                    render_inline_spans(spans, area.x, row, buf, area, false);
                    row += 1;
                }

                MarkdownNode::List { ordered, items } => {
                    for (idx, item) in items.iter().enumerate() {
                        if row >= area.y + area.height {
                            break;
                        }
                        let prefix = if *ordered {
                            format!("  {}. ", idx + 1)
                        } else {
                            "  \u{2022} ".to_string()
                        };
                        let mut cur_x = area.x;
                        let max_x = area.x + area.width;
                        for ch in prefix.chars() {
                            if cur_x >= max_x {
                                break;
                            }
                            let cell = buf.get_mut(cur_x, row);
                            cell.ch = ch;
                            cell.fg = Some(theme::DIM);
                            cell.bold = false;
                            cur_x += 1;
                        }
                        render_inline_spans(item, cur_x, row, buf, area, false);
                        row += 1;
                    }
                }

                MarkdownNode::Quote { spans } => {
                    let prefix = "\u{2502} "; // "│ "
                    let mut cur_x = area.x;
                    let max_x = area.x + area.width;
                    for ch in prefix.chars() {
                        if cur_x >= max_x {
                            break;
                        }
                        let cell = buf.get_mut(cur_x, row);
                        cell.ch = ch;
                        cell.fg = Some(Color::Rgb(100, 100, 100));
                        cell.bold = false;
                        cur_x += 1;
                    }
                    // Quote content rendered dim
                    for inline in spans {
                        let mut sp = inline_span_to_span(inline, false);
                        sp.fg = Some(theme::DIM);
                        sp.bold = false;
                        for ch in sp.text.chars() {
                            if cur_x >= max_x {
                                break;
                            }
                            let cell = buf.get_mut(cur_x, row);
                            cell.ch = ch;
                            cell.fg = sp.fg;
                            cell.bold = sp.bold;
                            cur_x += 1;
                        }
                        if cur_x >= max_x {
                            break;
                        }
                    }
                    row += 1;
                }

                MarkdownNode::CodeBlock { language, code } => {
                    let h = CodeBlockWidget::height(code, area.width);
                    let available = area.y + area.height - row;
                    let sub_area = Rect::new(area.x, row, area.width, h.min(available));
                    if !sub_area.is_empty() {
                        let widget = CodeBlockWidget::new(code, language.as_deref());
                        widget.render(sub_area, buf);
                    }
                    row = row.saturating_add(h);
                }

                MarkdownNode::HorizontalRule => {
                    let max_x = area.x + area.width;
                    let mut cur_x = area.x;
                    while cur_x < max_x {
                        let cell = buf.get_mut(cur_x, row);
                        cell.ch = '\u{2500}'; // '─'
                        cell.fg = Some(theme::DIM);
                        cell.bold = false;
                        cur_x += 1;
                    }
                    row += 1;
                }
            }
        }
    }
}

// ── backward compat ───────────────────────────────────────────────────────────

/// Render markdown text as a list of [`Line`]s.
///
/// Kept for backward compatibility — used by `message_style.rs`.
pub fn render_markdown(text: &str) -> Vec<Line> {
    let nodes = parse_md(text);
    if nodes.is_empty() {
        return vec![Line::from_spans(vec![Span::raw("")])];
    }

    let mut lines: Vec<Line> = Vec::new();

    for node in &nodes {
        match node {
            MarkdownNode::Heading { text, .. } => {
                let spans: Vec<Span> = text.iter().map(|s| inline_span_to_span(s, true)).collect();
                lines.push(Line::from_spans(spans));
            }

            MarkdownNode::Paragraph { spans } => {
                let line_spans: Vec<Span> = spans
                    .iter()
                    .map(|s| inline_span_to_span(s, false))
                    .collect();
                lines.push(Line::from_spans(line_spans));
            }

            MarkdownNode::List { ordered, items } => {
                for (idx, item) in items.iter().enumerate() {
                    let prefix = if *ordered {
                        format!("  {}. ", idx + 1)
                    } else {
                        "  \u{2022} ".to_string()
                    };
                    let mut spans = vec![Span::styled(prefix, theme::DIM, false)];
                    spans.extend(item.iter().map(|s| inline_span_to_span(s, false)));
                    lines.push(Line::from_spans(spans));
                }
            }

            MarkdownNode::Quote { spans } => {
                let mut line_spans =
                    vec![Span::styled("\u{2502} ", Color::Rgb(100, 100, 100), false)];
                line_spans.extend(spans.iter().map(|s| {
                    let mut sp = inline_span_to_span(s, false);
                    sp.fg = Some(theme::DIM);
                    sp.bold = false;
                    sp
                }));
                lines.push(Line::from_spans(line_spans));
            }

            MarkdownNode::CodeBlock { code, .. } => {
                if code.is_empty() {
                    lines.push(Line::from_spans(vec![Span::raw("")]));
                } else {
                    for code_line in code.lines() {
                        lines.push(Line::from_spans(vec![Span::raw(code_line)]));
                    }
                }
            }

            MarkdownNode::HorizontalRule => {
                lines.push(Line::from_spans(vec![Span::styled(
                    "\u{2500}".repeat(40),
                    theme::DIM,
                    false,
                )]));
            }
        }
    }

    if lines.is_empty() {
        lines.push(Line::from_spans(vec![Span::raw("")]));
    }

    lines
}

// Keep a local parse_inline_spans helper for any internal use
#[allow(dead_code)]
fn parse_inline_spans(line: &str) -> Vec<Span> {
    parse_inline_content(line)
        .iter()
        .map(|s| inline_span_to_span(s, false))
        .collect()
}
