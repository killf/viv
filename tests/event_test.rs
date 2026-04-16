use viv::event::*;

#[test]
fn epoll_event_struct_size() {
    assert_eq!(std::mem::size_of::<EpollEvent>(), 12);
}

#[test]
fn epoll_create_and_drop() {
    let epoll = Epoll::new().unwrap();
    // Just verify it doesn't crash. Drop will close.
    drop(epoll);
}

#[test]
fn epoll_wait_timeout() {
    let epoll = Epoll::new().unwrap();
    let ready = epoll.wait(1).unwrap(); // 1ms timeout
    assert!(ready.is_empty()); // nothing registered, nothing ready
}
