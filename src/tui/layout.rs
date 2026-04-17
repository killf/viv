use crate::core::terminal::buffer::Rect;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Constraint {
    Fixed(u16),
    Percentage(u16),
    Min(u16),
    Fill,
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub direction: Direction,
    pub constraints: Vec<Constraint>,
}

impl Layout {
    pub fn new(direction: Direction) -> Self {
        Layout { direction, constraints: Vec::new() }
    }

    pub fn constraints(mut self, c: Vec<Constraint>) -> Self {
        self.constraints = c;
        self
    }

    pub fn split(&self, area: Rect) -> Vec<Rect> {
        let total = match self.direction {
            Direction::Horizontal => area.width,
            Direction::Vertical => area.height,
        };

        let n = self.constraints.len();
        let mut sizes = vec![0u16; n];

        // First pass: resolve Fixed and Percentage; count Fill slots
        let mut used: u32 = 0;
        let mut fill_count: u32 = 0;

        for (i, c) in self.constraints.iter().enumerate() {
            match c {
                Constraint::Fixed(v) => {
                    sizes[i] = (*v).min(total.saturating_sub(used as u16));
                    used += sizes[i] as u32;
                }
                Constraint::Percentage(p) => {
                    let pct = (*p).min(100) as u32;
                    let v = (total as u32 * pct / 100) as u16;
                    sizes[i] = v.min(total.saturating_sub(used as u16));
                    used += sizes[i] as u32;
                }
                Constraint::Min(_) => {
                    // handled after Fill
                }
                Constraint::Fill => {
                    fill_count += 1;
                }
            }
        }

        // Second pass: handle Min constraints (give them at least their minimum)
        for (i, c) in self.constraints.iter().enumerate() {
            if let Constraint::Min(m) = c {
                let remaining = total.saturating_sub(used as u16);
                let v = (*m).min(total);
                if v <= remaining {
                    sizes[i] = v;
                } else {
                    sizes[i] = remaining;
                }
                used += sizes[i] as u32;
            }
        }

        // Third pass: distribute remaining space equally among Fill slots
        let remaining = total.saturating_sub(used as u16);
        if fill_count > 0 {
            let per_fill = remaining / fill_count as u16;
            let mut leftover = remaining - per_fill * fill_count as u16;
            for (i, c) in self.constraints.iter().enumerate() {
                if let Constraint::Fill = c {
                    sizes[i] = per_fill + if leftover > 0 { leftover -= 1; 1 } else { 0 };
                }
            }
        }

        // Build Rects by stacking
        let mut rects = Vec::with_capacity(n);
        let mut offset = 0u16;
        for size in sizes {
            let rect = match self.direction {
                Direction::Horizontal => {
                    Rect::new(area.x + offset, area.y, size, area.height)
                }
                Direction::Vertical => {
                    Rect::new(area.x, area.y + offset, area.width, size)
                }
            };
            rects.push(rect);
            offset = offset.saturating_add(size);
        }
        rects
    }
}
