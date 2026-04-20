#[derive(Debug)]
pub struct FocusManager {
    focus_index: usize,
    focusable_count: usize,
}

impl FocusManager {
    pub fn new() -> Self {
        FocusManager {
            focus_index: 0,
            focusable_count: 0,
        }
    }

    pub fn focus_index(&self) -> usize {
        self.focus_index
    }

    pub fn next(&mut self) {
        if self.focusable_count == 0 {
            return;
        }
        self.focus_index = (self.focus_index + 1) % self.focusable_count;
    }

    pub fn prev(&mut self) {
        if self.focusable_count == 0 {
            return;
        }
        if self.focus_index == 0 {
            self.focus_index = self.focusable_count - 1;
        } else {
            self.focus_index -= 1;
        }
    }

    pub fn is_focused(&self, index: usize) -> bool {
        self.focus_index == index
    }

    pub fn update_count(&mut self, count: usize) {
        self.focusable_count = count;
        if count == 0 {
            self.focus_index = 0;
        } else if self.focus_index >= count {
            self.focus_index = count - 1;
        }
    }
}
