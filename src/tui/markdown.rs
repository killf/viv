use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::style::Color;
use crate::core::terminal::style::theme;
use crate::tui::code_block::CodeBlockWidget;
use crate::tui::content::{
    InlineSpan, MarkdownNode, parse_inline as parse_inline_content, parse_markdown as parse_md,
};
use crate::tui::paragraph::{Line, Span, wrap_line};
use crate::tui::widget::Widget;

pub struct MarkdownBlockWidget<'a> {
    nodes: &'a [MarkdownNode],
}

impl<'a> MarkdownBlockWidget<'a> {
    pub fn new(nodes: &'a [MarkdownNode]) -> Self {
        MarkdownBlockWidget { nodes }
    }

    /// Sum of heights of all nodes given the available width, including
    /// one blank row between adjacent nodes.
    pub fn height(nodes: &[MarkdownNode], width: u16) -> u16 {
        let mut total: u16 = 0;
        for (i, node) in nodes.iter().enumerate() {
            total = total.saturating_add(node_height(node, width));
            if i + 1 < nodes.len() {
                total = total.saturating_add(1); // spacing between nodes
            }
        }
        total
    }
}

fn node_height(node: &MarkdownNode, width: u16) -> u16 {
    let w = width as usize;
    match node {
        MarkdownNode::Heading { text, .. } => wrap_line(&spans_to_line(text, true), w).len() as u16,
        MarkdownNode::Paragraph { spans } => {
            wrap_line(&spans_to_line(spans, false), w).len() as u16
        }
        MarkdownNode::List { ordered, items } => {
            let prefix_width = if *ordered { 5 } else { 4 }; // "  1. " or "  • "
            let inner_w = w.saturating_sub(prefix_width);
            items
                .iter()
                .map(|item| wrap_line(&spans_to_line(item, false), inner_w.max(1)).len() as u16)
                .sum()
        }
        MarkdownNode::Quote { spans } => {
            let inner_w = w.saturating_sub(2); // "│ " prefix
            wrap_line(&spans_to_line(spans, false), inner_w.max(1)).len() as u16
        }
        MarkdownNode::CodeBlock { code, .. } => CodeBlockWidget::height(code, width),
        MarkdownNode::HorizontalRule => 1,
    }
}

fn spans_to_line(spans: &[InlineSpan], bold_context: bool) -> Line {
    let line_spans: Vec<Span> = spans
        .iter()
        .map(|s| inline_span_to_span(s, bold_context))
        .collect();
    Line::from_spans(line_spans)
}

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
        InlineSpan::Bold(s) => Span::styled(s.clone(), theme::TEXT, true),
        InlineSpan::Italic(s) => Span::styled(s.clone(), theme::DIM, false),
        InlineSpan::Code(s) => Span::styled(s.clone(), Color::Rgb(215, 119, 87), false),
        InlineSpan::Link { text, .. } => {
            Span::styled(text.clone(), Color::Rgb(100, 150, 255), false)
        }
    }
}

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

impl<'a> Widget for MarkdownBlockWidget<'a> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        let mut row = area.y;
        let node_count = self.nodes.len();

        for (node_idx, node) in self.nodes.iter().enumerate() {
            if row >= area.y + area.height {
                break;
            }

            match node {
                MarkdownNode::Heading { text, .. } => {
                    let rows = render_wrapped_spans(text, area.x, row, buf, area, true, area.width);
                    row += rows;
                }

                MarkdownNode::Paragraph { spans } => {
                    let rows =
                        render_wrapped_spans(spans, area.x, row, buf, area, false, area.width);
                    row += rows;
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
                        let prefix_len = prefix.len() as u16;
                        let inner_width = area.width.saturating_sub(prefix_len);

                        // Render prefix on first row
                        let max_x = area.x + area.width;
                        for (i, ch) in prefix.chars().enumerate() {
                            let cx = area.x + i as u16;
                            if cx >= max_x {
                                break;
                            }
                            let cell = buf.get_mut(cx, row);
                            cell.ch = ch;
                            cell.fg = Some(theme::DIM);
                            cell.bold = false;
                        }

                        // Render wrapped content
                        let rows = render_wrapped_spans(
                            item,
                            area.x + prefix_len,
                            row,
                            buf,
                            area,
                            false,
                            inner_width.max(1),
                        );
                        // Continuation rows are already rendered at the correct
                        // indent by render_wrapped_spans; the prefix area on
                        // continuation rows is left blank (spaces from Buffer default).
                        row += rows;
                    }
                }

                MarkdownNode::Quote { spans } => {
                    let inner_width = area.width.saturating_sub(2);
                    let line = spans_to_line(spans, false);
                    let rows = wrap_line(&line, inner_width.max(1) as usize);
                    let mut rows_rendered: u16 = 0;
                    for (row_idx, wrapped_row) in rows.iter().enumerate() {
                        let y = row + row_idx as u16;
                        if y >= area.y + area.height {
                            break;
                        }
                        // Render "│ " prefix on each row
                        let prefix = "\u{2502} ";
                        let mut cur_x = area.x;
                        let max_x = area.x + area.width;
                        for ch in prefix.chars() {
                            if cur_x >= max_x {
                                break;
                            }
                            let cell = buf.get_mut(cur_x, y);
                            cell.ch = ch;
                            cell.fg = Some(Color::Rgb(100, 100, 100));
                            cell.bold = false;
                            cur_x += 1;
                        }
                        // Render content with dim styling
                        for sc in wrapped_row {
                            if sc.width == 0 {
                                continue;
                            }
                            if cur_x + sc.width > max_x {
                                break;
                            }
                            let cell = buf.get_mut(cur_x, y);
                            cell.ch = sc.ch;
                            cell.fg = Some(theme::DIM);
                            cell.bg = sc.bg;
                            cell.bold = false;
                            cell.italic = sc.italic;
                            cell.dim = sc.dim;
                            if sc.width == 2 && cur_x + 1 < max_x {
                                let cell2 = buf.get_mut(cur_x + 1, y);
                                cell2.ch = '\0';
                                cell2.fg = Some(theme::DIM);
                                cell2.bg = sc.bg;
                                cell2.bold = false;
                                cell2.italic = sc.italic;
                                cell2.dim = sc.dim;
                            }
                            cur_x += sc.width;
                        }
                        rows_rendered += 1;
                    }
                    row += rows_rendered;
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

            // Add spacing between nodes (not after the last)
            if node_idx + 1 < node_count && row < area.y + area.height {
                row += 1;
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
