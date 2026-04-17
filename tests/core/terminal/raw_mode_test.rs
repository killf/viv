use viv::core::terminal::raw_mode::*;

#[test]
fn termios_struct_size() {
    assert_eq!(std::mem::size_of::<Termios>(), 60);
}

#[test]
fn raw_mode_flags() {
    assert_eq!(ECHO, 0o10);
    assert_eq!(ICANON, 0o2);
    assert_eq!(ISIG, 0o1);
}

#[test]
fn raw_mode_modifies_flags() {
    let mut termios = Termios {
        c_iflag: IXON | ICRNL | 0xFF,
        c_oflag: OPOST | 0xFF,
        c_cflag: 0,
        c_lflag: ECHO | ICANON | ISIG | IEXTEN | 0xFF,
        c_line: 0,
        c_cc: [0; 32],
        c_ispeed: 0,
        c_ospeed: 0,
    };
    apply_raw_flags(&mut termios);
    assert_eq!(termios.c_lflag & ECHO, 0);
    assert_eq!(termios.c_lflag & ICANON, 0);
    assert_eq!(termios.c_lflag & ISIG, 0);
    assert_eq!(termios.c_lflag & IEXTEN, 0);
    assert_eq!(termios.c_iflag & IXON, 0);
    assert_eq!(termios.c_iflag & ICRNL, 0);
    assert_eq!(termios.c_oflag & OPOST, 0);
}
