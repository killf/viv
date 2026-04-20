use viv::tui::selection::{SelectionRegion, SelectionState};

#[test]
fn test_selection_normalize_same_point() {
    let region = SelectionRegion::normalize((10, 20), (10, 20));
    assert_eq!(region.top_left, (10, 20));
    assert_eq!(region.bottom_right, (10, 20));
}

#[test]
fn test_selection_normalize_different_points() {
    let region = SelectionRegion::normalize((40, 10), (20, 30));
    assert_eq!(region.top_left, (20, 10));
    assert_eq!(region.bottom_right, (40, 30));
}

#[test]
fn test_selection_contains_inside() {
    let region = SelectionRegion::normalize((10, 10), (20, 20));
    assert!(region.contains((15, 15)));
}

#[test]
fn test_selection_contains_on_boundary() {
    let region = SelectionRegion::normalize((10, 10), (20, 20));
    assert!(region.contains((10, 10)));
    assert!(region.contains((20, 20)));
}

#[test]
fn test_selection_contains_outside() {
    let region = SelectionRegion::normalize((10, 10), (20, 20));
    assert!(!region.contains((9, 15)));
    assert!(!region.contains((21, 15)));
    assert!(!region.contains((15, 9)));
    assert!(!region.contains((15, 21)));
}

#[test]
fn test_selection_as_rect() {
    let region = SelectionRegion::normalize((10, 5), (20, 15));
    let rect = region.as_rect();
    assert_eq!(rect.x, 10);
    assert_eq!(rect.y, 5);
    assert_eq!(rect.width, 11);  // 20 - 10 + 1
    assert_eq!(rect.height, 11); // 15 - 5 + 1
}

#[test]
fn test_selection_new() {
    let state = SelectionState::new();
    assert!(!state.has_selection());
    assert!(!state.is_dragging());
    assert!(state.region().is_none());
}

#[test]
fn test_selection_start_drag() {
    let mut state = SelectionState::new();
    state.start_drag(40, 10);
    assert!(state.is_dragging());
    assert!(!state.has_selection()); // dragging = not yet a valid selection
    assert_eq!(state.region(), Some(SelectionRegion::normalize((40, 10), (40, 10))));
}

#[test]
fn test_selection_update_drag() {
    let mut state = SelectionState::new();
    state.start_drag(40, 10);
    state.update_drag(60, 20);
    assert!(state.is_dragging());
    assert_eq!(state.region(), Some(SelectionRegion::normalize((40, 10), (60, 20))));
}

#[test]
fn test_selection_end_drag() {
    let mut state = SelectionState::new();
    state.start_drag(40, 10);
    state.update_drag(60, 20);
    state.end_drag(60, 20);
    assert!(!state.is_dragging());
    assert!(state.has_selection()); // drag ended = valid selection
    assert_eq!(state.region(), Some(SelectionRegion::normalize((40, 10), (60, 20))));
}

#[test]
fn test_selection_clear() {
    let mut state = SelectionState::new();
    state.start_drag(40, 10);
    state.end_drag(60, 20);
    state.clear();
    assert!(!state.has_selection());
    assert!(!state.is_dragging());
    assert!(state.region().is_none());
}

#[test]
fn test_full_selection_flow() {
    let mut state = SelectionState::new();

    // 1. User presses mouse at (40, 10)
    state.start_drag(40, 10);
    assert!(state.is_dragging());
    assert!(!state.has_selection());

    // 2. User drags to (60, 20)
    state.update_drag(60, 20);
    assert!(state.is_dragging());
    assert_eq!(state.region(), Some(SelectionRegion::normalize((40, 10), (60, 20))));

    // 3. User releases mouse
    state.end_drag(60, 20);
    assert!(!state.is_dragging());
    assert!(state.has_selection());

    // 4. Verify selection area
    let region = state.region().unwrap();
    assert!(region.contains((50, 15)));  // inside
    assert!(!region.contains((30, 15))); // outside
}

#[test]
fn test_reverse_drag_direction() {
    let mut state = SelectionState::new();
    state.start_drag(60, 20);
    state.update_drag(40, 10);
    state.end_drag(40, 10);
    let region = state.region().unwrap();
    assert_eq!(region.top_left, (40, 10));
    assert_eq!(region.bottom_right, (60, 20));
}

#[test]
fn test_selection_clear_on_scroll() {
    let mut state = SelectionState::new();
    state.start_drag(40, 10);
    state.end_drag(60, 20);
    assert!(state.has_selection());
    state.clear();
    assert!(!state.has_selection());
    assert!(state.region().is_none());
}
