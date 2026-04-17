use viv::tui::layout::*;
use viv::core::terminal::buffer::Rect;

#[test]
fn split_two_fixed_vertical() {
    let rects = Layout::new(Direction::Vertical)
        .constraints(vec![Constraint::Fixed(3), Constraint::Fixed(5)])
        .split(Rect::new(0, 0, 80, 24));
    assert_eq!(rects.len(), 2);
    assert_eq!(rects[0], Rect::new(0, 0, 80, 3));
    assert_eq!(rects[1], Rect::new(0, 3, 80, 5));
}

#[test]
fn split_two_fixed_horizontal() {
    let rects = Layout::new(Direction::Horizontal)
        .constraints(vec![Constraint::Fixed(20), Constraint::Fixed(30)])
        .split(Rect::new(0, 0, 80, 24));
    assert_eq!(rects.len(), 2);
    assert_eq!(rects[0], Rect::new(0, 0, 20, 24));
    assert_eq!(rects[1], Rect::new(20, 0, 30, 24));
}

#[test]
fn split_fill_takes_remainder() {
    let rects = Layout::new(Direction::Vertical)
        .constraints(vec![Constraint::Fixed(3), Constraint::Fill])
        .split(Rect::new(0, 0, 80, 24));
    assert_eq!(rects[0], Rect::new(0, 0, 80, 3));
    assert_eq!(rects[1], Rect::new(0, 3, 80, 21));
}

#[test]
fn split_percentage() {
    let rects = Layout::new(Direction::Vertical)
        .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(Rect::new(0, 0, 80, 24));
    assert_eq!(rects[0].height, 12);
    assert_eq!(rects[1].height, 12);
}

#[test]
fn split_min_constraint() {
    let rects = Layout::new(Direction::Vertical)
        .constraints(vec![Constraint::Min(5), Constraint::Fixed(3)])
        .split(Rect::new(0, 0, 80, 24));
    assert!(rects[0].height >= 5);
    assert_eq!(rects[1].height, 3);
}

#[test]
fn split_empty_area() {
    let rects = Layout::new(Direction::Vertical)
        .constraints(vec![Constraint::Fill])
        .split(Rect::new(0, 0, 0, 0));
    assert_eq!(rects.len(), 1);
    assert!(rects[0].is_empty());
}

#[test]
fn split_three_way() {
    let rects = Layout::new(Direction::Vertical)
        .constraints(vec![Constraint::Fixed(1), Constraint::Fill, Constraint::Fixed(1)])
        .split(Rect::new(0, 0, 80, 24));
    assert_eq!(rects[0].height, 1);
    assert_eq!(rects[1].height, 22);
    assert_eq!(rects[2].height, 1);
}
