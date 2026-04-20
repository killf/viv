pub mod notifier;
pub mod process;
pub mod reactor;
pub mod terminal;
pub mod timer;

pub use notifier::PipeNotifier;
pub use process::shell_command;
pub use reactor::EpollReactor;
pub use terminal::{terminal_size, UnixResizeListener, UnixTerminal};
pub use timer::UnixTimer;

pub fn tcp_raw_handle(stream: &std::net::TcpStream) -> super::types::RawHandle {
    use std::os::unix::io::AsRawFd;
    stream.as_raw_fd()
}
