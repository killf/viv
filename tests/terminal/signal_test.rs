use viv::terminal::signal::*;

#[test]
fn signal_pipe_creation() {
    let pipe = SignalPipe::new().unwrap();
    assert!(pipe.read_fd() >= 0);
}

#[test]
fn drain_on_empty_is_ok() {
    let pipe = SignalPipe::new().unwrap();
    assert!(pipe.drain().is_ok());
}

#[test]
fn sigaction_struct_size() {
    // Linux x86_64: sa_handler(8) + sa_flags(8) + sa_restorer(8) + sa_mask(128) = 152
    assert_eq!(std::mem::size_of::<Sigaction>(), 152);
}
