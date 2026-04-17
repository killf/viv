use viv::core::terminal::buffer::{Rect, Buffer};
use viv::tui::widget::Widget;

struct TestWidget {
    text: &'static str,
}

impl Widget for TestWidget {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        buf.set_str(area.x, area.y, self.text, None, false);
    }
}

#[test]
fn widget_trait_renders() {
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
    let w = TestWidget { text: "hello" };
    w.render(Rect::new(0, 0, 20, 5), &mut buf);
    assert_eq!(buf.get(0, 0).ch, 'h');
    assert_eq!(buf.get(4, 0).ch, 'o');
}
