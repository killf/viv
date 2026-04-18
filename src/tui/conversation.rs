use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::style::Color;

/// Describes a single item's visibility within the viewport.
#[derive(Debug, Clone)]
pub struct VisibleItem {
    /// Index into the blocks list.
    pub index: usize,
    /// Rows clipped from the top of this item.
    pub clip_top: u16,
    /// How many rows of this item are visible.
    pub visible_rows: u16,
    /// Y position in the viewport where this item starts rendering.
    pub viewport_y: u16,
}

/// Manages scroll state and height caching for the virtual scroll conversation view.
#[derive(Debug)]
pub struct ConversationState {
    /// Current scroll offset (in rows).
    pub scroll_offset: u16,
    /// Height of the visible viewport.
    pub viewport_height: u16,
    /// When true, automatically scroll to the bottom as new content arrives.
    pub auto_follow: bool,
    /// Cached heights of each item.
    pub item_heights: Vec<u16>,
    /// Sum of all item heights.
    pub total_height: u16,
}

impl ConversationState {
    pub fn new() -> Self {
        ConversationState {
            scroll_offset: 0,
            viewport_height: 0,
            auto_follow: true,
            item_heights: Vec::new(),
            total_height: 0,
        }
    }

    /// Maximum scroll offset: total content height minus viewport height.
    fn max_scroll(&self) -> u16 {
        self.total_height.saturating_sub(self.viewport_height)
    }

    /// Append a new item height and update the total.
    pub fn append_item_height(&mut self, height: u16) {
        self.item_heights.push(height);
        self.total_height = self.total_height.saturating_add(height);
    }

    /// Update an existing item's height and adjust the total.
    pub fn set_item_height(&mut self, index: usize, height: u16) {
        if index < self.item_heights.len() {
            let old = self.item_heights[index];
            self.item_heights[index] = height;
            self.total_height = self.total_height.saturating_sub(old).saturating_add(height);
        }
    }

    /// Update the last item's height and adjust the total.
    pub fn update_last_height(&mut self, height: u16) {
        if let Some(last) = self.item_heights.last_mut() {
            let old = *last;
            *last = height;
            self.total_height = self.total_height.saturating_sub(old).saturating_add(height);
        }
    }

    /// Recompute total_height from scratch from item_heights.
    pub fn recalculate_total(&mut self) {
        self.total_height = self
            .item_heights
            .iter()
            .copied()
            .fold(0u16, |acc, h| acc.saturating_add(h));
    }

    /// If auto_follow is enabled, scroll to the bottom.
    pub fn auto_scroll(&mut self) {
        if self.auto_follow {
            self.scroll_offset = self.max_scroll();
        }
    }

    /// Scroll up by `lines` rows. Disables auto_follow.
    pub fn scroll_up(&mut self, lines: u16) {
        self.auto_follow = false;
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    /// Scroll down by `lines` rows. Disables auto_follow.
    pub fn scroll_down(&mut self, lines: u16) {
        self.auto_follow = false;
        let new_offset = self.scroll_offset.saturating_add(lines);
        self.scroll_offset = new_offset.min(self.max_scroll());
    }

    /// Scroll up by one page (viewport_height - 2).
    pub fn page_up(&mut self) {
        let lines = self.viewport_height.saturating_sub(2);
        self.scroll_up(lines);
    }

    /// Scroll down by one page (viewport_height - 2).
    pub fn page_down(&mut self) {
        let lines = self.viewport_height.saturating_sub(2);
        self.scroll_down(lines);
    }

    /// Jump to the top. Disables auto_follow.
    pub fn scroll_to_top(&mut self) {
        self.auto_follow = false;
        self.scroll_offset = 0;
    }

    /// Jump to the bottom. Re-enables auto_follow.
    pub fn scroll_to_bottom(&mut self) {
        self.auto_follow = true;
        self.scroll_offset = self.max_scroll();
    }

    /// Compute which items overlap the current viewport window.
    pub fn visible_items(&self) -> Vec<VisibleItem> {
        let mut result = Vec::new();
        let viewport_end = self.scroll_offset.saturating_add(self.viewport_height);
        let mut cumulative_y: u16 = 0;

        for (index, &height) in self.item_heights.iter().enumerate() {
            let item_start = cumulative_y;
            let item_end = cumulative_y.saturating_add(height);
            cumulative_y = item_end;

            // Skip items entirely before the viewport
            if item_end <= self.scroll_offset {
                continue;
            }

            // Stop once we're past the viewport
            if item_start >= viewport_end {
                break;
            }

            // This item overlaps the viewport
            let clip_top = if self.scroll_offset > item_start {
                self.scroll_offset - item_start
            } else {
                0
            };

            let visible_start = item_start.max(self.scroll_offset);
            let visible_end = item_end.min(viewport_end);
            let visible_rows = visible_end.saturating_sub(visible_start);
            let viewport_y = visible_start.saturating_sub(self.scroll_offset);

            result.push(VisibleItem {
                index,
                clip_top,
                visible_rows,
                viewport_y,
            });
        }

        result
    }

    /// Render a scrollbar into the rightmost column of `area`.
    pub fn render_scrollbar(&self, area: Rect, buf: &mut Buffer) {
        if self.total_height <= self.viewport_height {
            return;
        }
        if area.is_empty() {
            return;
        }

        let bar_x = area.x + area.width.saturating_sub(1);
        let bar_height = area.height as u32;

        // Compute thumb size and position (with integer arithmetic to avoid float)
        let thumb_size =
            ((self.viewport_height as u32 * bar_height) / self.total_height as u32).max(1) as u16;
        let thumb_pos =
            ((self.scroll_offset as u32 * bar_height) / self.total_height as u32) as u16;

        let thumb_color = Color::Rgb(180, 180, 180);
        let track_color = Color::Rgb(60, 60, 60);

        for row in 0..area.height {
            let y = area.y + row;
            let cell = buf.get_mut(bar_x, y);
            if row >= thumb_pos && row < thumb_pos + thumb_size {
                cell.ch = '┃';
                cell.fg = Some(thumb_color);
            } else {
                cell.ch = '│';
                cell.fg = Some(track_color);
            }
        }
    }
}

impl Default for ConversationState {
    fn default() -> Self {
        ConversationState::new()
    }
}
