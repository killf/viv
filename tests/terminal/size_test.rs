use viv::terminal::size::*;

#[test]
fn terminal_size_returns_valid() {
    let size = terminal_size().unwrap();
    assert!(size.cols > 0);
    assert!(size.rows > 0);
}

#[test]
fn term_size_copy_and_eq() {
    let a = TermSize { cols: 80, rows: 24 };
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn winsize_struct_layout() {
    assert_eq!(std::mem::size_of::<Winsize>(), 8);
}
