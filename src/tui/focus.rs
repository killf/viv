#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UIMode {
    Normal, // keyboard input goes to LineEditor
    Browse, // keyboard navigates tool calls (Esc to enter, Esc to exit)
}

#[derive(Debug)]
pub struct FocusManager {
    mode: UIMode,
    focus_index: usize,
    focusable_count: usize,
}

impl FocusManager {
    pub fn new() -> Self {
        FocusManager {
            mode: UIMode::Normal,
            focus_index: 0,
            focusable_count: 0,
        }
    }

    pub fn mode(&self) -> UIMode {
        self.mode
    }

    pub fn focus_index(&self) -> usize {
        self.focus_index
    }

    pub fn enter_browse(&mut self, focusable_count: usize) {
        self.mode = UIMode::Browse;
        self.focusable_count = focusable_count;
        if focusable_count > 0 && self.focus_index >= focusable_count {
            self.focus_index = focusable_count - 1;
        } else if focusable_count == 0 {
            self.focus_index = 0;
        }
    }

    pub fn exit_browse(&mut self) {
        self.mode = UIMode::Normal;
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
        self.mode == UIMode::Browse && self.focus_index == index
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
