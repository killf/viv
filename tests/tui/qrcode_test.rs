use viv::core::terminal::buffer::{Buffer, Rect};
use viv::tui::qrcode::QrCodeWidget;
use viv::tui::widget::Widget;

#[test]
fn renders_without_panic() {
    let widget = QrCodeWidget::new("test");
    let area = Rect::new(0, 0, 40, 20);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
}

#[test]
fn height_short_text() {
    let h = QrCodeWidget::height("Hi");
    // V1: 21 + 4 quiet = 25, /2 ceil = 13
    assert!(h >= 10 && h <= 20);
}

#[test]
fn renders_half_block_chars() {
    let widget = QrCodeWidget::new("A");
    let area = Rect::new(0, 0, 40, 20);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    let has_blocks = (0..area.width).any(|x| {
        (0..area.height).any(|y| buf.get(x, y).ch == '▀')
    });
    assert!(has_blocks, "QR code should use ▀ characters");
}

#[test]
fn renders_centered_in_large_area() {
    let widget = QrCodeWidget::new("Hi");
    let area = Rect::new(0, 0, 80, 30);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    // Column 0 should be empty (default space) since QR is centered
    let first_col_default = (0..area.height).all(|y| buf.get(0, y).ch == ' ');
    assert!(first_col_default, "QR should be centered, edges empty");
}

#[test]
fn too_small_area_no_panic() {
    let widget = QrCodeWidget::new("test");
    let area = Rect::new(0, 0, 3, 2);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf); // should not panic
}

#[test]
fn height_longer_text() {
    let h1 = QrCodeWidget::height("Hi");
    let h2 = QrCodeWidget::height(&"a".repeat(100));
    assert!(h2 > h1, "longer text should produce taller QR code");
}
