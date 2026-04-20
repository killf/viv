#[path = "platform/notifier_test.rs"]
mod notifier_test;
#[path = "platform/process_test.rs"]
mod process_test;
#[path = "platform/reactor_test.rs"]
mod reactor_test;
#[path = "platform/timer_test.rs"]
mod timer_test;
#[cfg(target_os = "linux")]
#[path = "platform/io_uring_test.rs"]
mod io_uring_test;
