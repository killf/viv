use viv::tui::conversation::ConversationState;

#[test]
fn initial_state_auto_follows() {
    let state = ConversationState::new();
    assert!(state.auto_follow);
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn append_height_updates_total() {
    let mut state = ConversationState::new();
    state.append_item_height(3);
    state.append_item_height(5);
    assert_eq!(state.total_height, 8);
    assert_eq!(state.item_heights.len(), 2);
}

#[test]
fn auto_follow_scrolls_to_bottom() {
    let mut state = ConversationState::new();
    state.viewport_height = 10;
    state.append_item_height(15);
    state.auto_scroll();
    // max_scroll = 15 - 10 = 5
    assert_eq!(state.scroll_offset, 5);
}

#[test]
fn manual_scroll_disables_auto_follow() {
    let mut state = ConversationState::new();
    state.viewport_height = 10;
    state.append_item_height(20);
    state.auto_scroll(); // sets offset to 10
    assert!(state.auto_follow);
    state.scroll_up(3);
    assert!(!state.auto_follow);
}

#[test]
fn scroll_to_bottom_restores_auto_follow() {
    let mut state = ConversationState::new();
    state.viewport_height = 10;
    state.append_item_height(20);
    state.scroll_up(5);
    assert!(!state.auto_follow);
    state.scroll_to_bottom();
    assert!(state.auto_follow);
}

#[test]
fn visible_range_skips_offscreen() {
    // 3 items of height 5, scroll_offset = 5
    // item 0: y=[0,5), item 1: y=[5,10), item 2: y=[10,15)
    // scroll_offset=5, viewport_height=5 → only item 1 is visible
    let mut state = ConversationState::new();
    state.viewport_height = 5;
    state.append_item_height(5);
    state.append_item_height(5);
    state.append_item_height(5);
    state.scroll_offset = 5;
    let items = state.visible_items();
    assert!(!items.is_empty());
    // The first visible item should be index 1
    assert_eq!(items[0].index, 1);
}

#[test]
fn recalculate_on_resize() {
    let mut state = ConversationState::new();
    state.append_item_height(5);
    state.append_item_height(5);
    assert_eq!(state.total_height, 10);
    // Update second item's height
    state.set_item_height(1, 10);
    assert_eq!(state.total_height, 15);
}

#[test]
fn page_down() {
    let mut state = ConversationState::new();
    state.viewport_height = 10;
    // total = 30, max_scroll = 20
    state.append_item_height(30);
    // page_down scrolls viewport_height - 2 = 8
    state.page_down();
    assert_eq!(state.scroll_offset, 8);
}

#[test]
fn scroll_does_not_go_negative() {
    let mut state = ConversationState::new();
    state.viewport_height = 10;
    state.append_item_height(5);
    // offset starts at 0, scroll_up(100) should stay at 0
    state.scroll_up(100);
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn update_last_height() {
    let mut state = ConversationState::new();
    state.append_item_height(5);
    state.append_item_height(5);
    assert_eq!(state.total_height, 10);
    state.update_last_height(10);
    assert_eq!(state.total_height, 15);
}

#[test]
fn visible_items_clip_top() {
    // item 0 has height 10, scroll into the middle
    // scroll_offset = 3, viewport_height = 10
    // item 0: starts at 0, ends at 10 → clip_top = 3
    let mut state = ConversationState::new();
    state.viewport_height = 10;
    state.append_item_height(10);
    state.append_item_height(10);
    state.scroll_offset = 3;
    let items = state.visible_items();
    assert!(!items.is_empty());
    assert_eq!(items[0].index, 0);
    assert_eq!(items[0].clip_top, 3);
}
