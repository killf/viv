use viv::terminal::output::*;

#[test]
fn cursor_move() {
    let mut w = AnsiWriter::new();
    w.move_to(5, 10);
    assert_eq!(w.take(), b"\x1b[6;11H");
}

#[test]
fn clear_screen() {
    let mut w = AnsiWriter::new();
    w.clear_screen();
    assert_eq!(w.take(), b"\x1b[2J");
}

#[test]
fn clear_line() {
    let mut w = AnsiWriter::new();
    w.clear_line();
    assert_eq!(w.take(), b"\x1b[2K");
}

#[test]
fn colors() {
    let mut w = AnsiWriter::new();
    w.fg_color(Color::Green);
    w.write_str("ok");
    w.reset_style();
    assert_eq!(w.take(), b"\x1b[32mok\x1b[0m");
}

#[test]
fn bold() {
    let mut w = AnsiWriter::new();
    w.bold();
    w.write_str("hi");
    w.reset_style();
    assert_eq!(w.take(), b"\x1b[1mhi\x1b[0m");
}

#[test]
fn cursor_visibility() {
    let mut w = AnsiWriter::new();
    w.hide_cursor();
    w.show_cursor();
    assert_eq!(w.take(), b"\x1b[?25l\x1b[?25h");
}
