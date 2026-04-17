use viv::tui::block::*;
use viv::tui::widget::Widget;
use viv::terminal::buffer::{Rect, Buffer};

#[test]
fn no_border_inner_equals_area() {
    let b = Block::new();
    let area = Rect::new(0, 0, 20, 10);
    assert_eq!(b.inner(area), area);
}

#[test]
fn plain_border_inner_shrinks() {
    let b = Block::new().border(BorderStyle::Plain);
    let inner = b.inner(Rect::new(0, 0, 20, 10));
    assert_eq!(inner, Rect::new(1, 1, 18, 8));
}

#[test]
fn renders_plain_corners() {
    let b = Block::new().border(BorderStyle::Plain);
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 5));
    b.render(Rect::new(0, 0, 10, 5), &mut buf);
    assert_eq!(buf.get(0, 0).ch, '┌');
    assert_eq!(buf.get(9, 0).ch, '┐');
    assert_eq!(buf.get(0, 4).ch, '└');
    assert_eq!(buf.get(9, 4).ch, '┘');
}

#[test]
fn renders_rounded_corners() {
    let b = Block::new().border(BorderStyle::Rounded);
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 5));
    b.render(Rect::new(0, 0, 10, 5), &mut buf);
    assert_eq!(buf.get(0, 0).ch, '╭');
    assert_eq!(buf.get(9, 0).ch, '╮');
    assert_eq!(buf.get(0, 4).ch, '╰');
    assert_eq!(buf.get(9, 4).ch, '╯');
}

#[test]
fn renders_horizontal_lines() {
    let b = Block::new().border(BorderStyle::Plain);
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 5));
    b.render(Rect::new(0, 0, 10, 5), &mut buf);
    assert_eq!(buf.get(1, 0).ch, '─');
    assert_eq!(buf.get(5, 0).ch, '─');
    assert_eq!(buf.get(1, 4).ch, '─');
}

#[test]
fn renders_vertical_lines() {
    let b = Block::new().border(BorderStyle::Plain);
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 5));
    b.render(Rect::new(0, 0, 10, 5), &mut buf);
    assert_eq!(buf.get(0, 1).ch, '│');
    assert_eq!(buf.get(0, 3).ch, '│');
    assert_eq!(buf.get(9, 2).ch, '│');
}

#[test]
fn renders_title() {
    let b = Block::new().border(BorderStyle::Plain).title("Test");
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
    b.render(Rect::new(0, 0, 20, 5), &mut buf);
    assert_eq!(buf.get(1, 0).ch, 'T');
    assert_eq!(buf.get(2, 0).ch, 'e');
    assert_eq!(buf.get(3, 0).ch, 's');
    assert_eq!(buf.get(4, 0).ch, 't');
}

#[test]
fn no_border_renders_nothing() {
    let b = Block::new();
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 5));
    b.render(Rect::new(0, 0, 10, 5), &mut buf);
    assert_eq!(buf.get(0, 0).ch, ' ');
}

// ── Selective border sides (Claude Code style: only top + bottom) ────────────

#[test]
fn borders_horizontal_only_inner_shrinks_vertically() {
    // Only top and bottom borders: inner area is full width, height - 2
    let b = Block::new()
        .border(BorderStyle::Rounded)
        .borders(BorderSides::HORIZONTAL);
    let inner = b.inner(Rect::new(0, 0, 20, 5));
    assert_eq!(inner, Rect::new(0, 1, 20, 3));
}

#[test]
fn horizontal_borders_draw_no_side_walls() {
    let b = Block::new()
        .border(BorderStyle::Rounded)
        .borders(BorderSides::HORIZONTAL);
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 5));
    b.render(Rect::new(0, 0, 10, 5), &mut buf);
    // Top and bottom rows should have horizontal lines, full width
    assert_eq!(buf.get(0, 0).ch, '─');
    assert_eq!(buf.get(5, 0).ch, '─');
    assert_eq!(buf.get(9, 0).ch, '─');
    assert_eq!(buf.get(0, 4).ch, '─');
    assert_eq!(buf.get(9, 4).ch, '─');
    // Left/right columns on middle rows have NO vertical lines
    assert_eq!(buf.get(0, 2).ch, ' ');
    assert_eq!(buf.get(9, 2).ch, ' ');
}

#[test]
fn horizontal_borders_title_inline() {
    let b = Block::new()
        .border(BorderStyle::Rounded)
        .borders(BorderSides::HORIZONTAL)
        .title(" main ");
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 3));
    b.render(Rect::new(0, 0, 30, 3), &mut buf);
    // Title should be on the top row somewhere
    let top_row: String = (0..30).map(|x| buf.get(x, 0).ch).collect();
    assert!(top_row.contains("main"), "top row should contain title: {top_row:?}");
    // Title should be surrounded by horizontal lines
    assert_eq!(buf.get(0, 0).ch, '─');
}

#[test]
fn default_borders_is_all_sides() {
    // Default Block::border() (without .borders()) uses ALL sides — backward compatible
    let b = Block::new().border(BorderStyle::Rounded);
    let inner = b.inner(Rect::new(0, 0, 20, 10));
    assert_eq!(inner, Rect::new(1, 1, 18, 8));
}
