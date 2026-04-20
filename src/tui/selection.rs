use crate::core::terminal::buffer::Rect;

/// SelectionRegion: normalized selection area (always top-left to bottom-right)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRegion {
    pub top_left: (u16, u16),
    pub bottom_right: (u16, u16),
}

impl SelectionRegion {
    /// Normalize any two points to top-left + bottom-right
    pub fn normalize(p1: (u16, u16), p2: (u16, u16)) -> Self {
        SelectionRegion {
            top_left: (p1.0.min(p2.0), p1.1.min(p2.1)),
            bottom_right: (p1.0.max(p2.0), p1.1.max(p2.1)),
        }
    }

    /// Check if a cell (col, row) is within the selection
    pub fn contains(&self, cell: (u16, u16)) -> bool {
        cell.0 >= self.top_left.0
            && cell.0 <= self.bottom_right.0
            && cell.1 >= self.top_left.1
            && cell.1 <= self.bottom_right.1
    }

    /// Convert to Rect for buffer iteration
    pub fn as_rect(&self) -> Rect {
        Rect {
            x: self.top_left.0,
            y: self.top_left.1,
            width: self.bottom_right.0.saturating_sub(self.top_left.0).saturating_add(1),
            height: self.bottom_right.1.saturating_sub(self.top_left.1).saturating_add(1),
        }
    }
}

/// SelectionState: manages drag-to-select state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionState {
    start_pos: Option<(u16, u16)>,
    end_pos: Option<(u16, u16)>,
    is_dragging: bool,
}

impl SelectionState {
    pub fn new() -> Self {
        SelectionState {
            start_pos: None,
            end_pos: None,
            is_dragging: false,
        }
    }

    pub fn start_drag(&mut self, x: u16, y: u16) {
        self.start_pos = Some((x, y));
        self.end_pos = Some((x, y));
        self.is_dragging = true;
    }

    pub fn update_drag(&mut self, x: u16, y: u16) {
        if self.is_dragging {
            self.end_pos = Some((x, y));
        }
    }

    pub fn end_drag(&mut self, x: u16, y: u16) {
        if self.is_dragging {
            self.end_pos = Some((x, y));
            self.is_dragging = false;
        }
    }

    /// Returns normalized region, or None if no selection
    pub fn region(&self) -> Option<SelectionRegion> {
        match (self.start_pos, self.end_pos) {
            (Some(start), Some(end)) => Some(SelectionRegion::normalize(start, end)),
            _ => None,
        }
    }

    /// True when drag has ended and there's a valid selection
    pub fn has_selection(&self) -> bool {
        self.start_pos.is_some() && self.end_pos.is_some() && !self.is_dragging
    }

    pub fn clear(&mut self) {
        self.start_pos = None;
        self.end_pos = None;
        self.is_dragging = false;
    }

    pub fn is_dragging(&self) -> bool {
        self.is_dragging
    }
}

impl Default for SelectionState {
    fn default() -> Self {
        Self::new()
    }
}
