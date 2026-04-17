use crate::core::terminal::buffer::{Rect, Buffer};

pub trait Widget {
    fn render(&self, area: Rect, buf: &mut Buffer);
}

pub trait StatefulWidget {
    type State;
    fn render(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State);
}
