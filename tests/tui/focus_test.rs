use viv::tui::focus::FocusManager;

#[test]
fn initial_state() {
    let fm = FocusManager::new();
    assert_eq!(fm.focus_index(), 0);
}

#[test]
fn next_focus_wraps() {
    let mut fm = FocusManager::new();
    fm.update_count(3);
    fm.next();
    assert_eq!(fm.focus_index(), 1);
    fm.next();
    assert_eq!(fm.focus_index(), 2);
    fm.next();
    assert_eq!(fm.focus_index(), 0);
}

#[test]
fn prev_focus_wraps() {
    let mut fm = FocusManager::new();
    fm.update_count(3);
    fm.prev();
    assert_eq!(fm.focus_index(), 2);
}

#[test]
fn zero_focusable_stays_at_zero() {
    let mut fm = FocusManager::new();
    fm.update_count(0);
    fm.next();
    assert_eq!(fm.focus_index(), 0);
}

#[test]
fn is_focused() {
    let mut fm = FocusManager::new();
    fm.update_count(3);
    assert!(fm.is_focused(0));
    fm.next();
    assert!(fm.is_focused(1));
    fm.next();
    assert!(fm.is_focused(2));
}

#[test]
fn update_count_bounds_focus() {
    let mut fm = FocusManager::new();
    fm.update_count(5);
    fm.next();
    fm.update_count(2);
    assert!(fm.is_focused(1));
}
