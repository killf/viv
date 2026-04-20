pub mod ffi;
pub mod notifier;
pub mod process;
pub mod reactor;
pub mod terminal;
pub mod timer;

pub use notifier::EventNotifier;
pub use process::{shell_command, spawn_piped, spawn_piped_with_env, ChildProcess};
pub use reactor::IocpReactor;
pub use terminal::{terminal_size, WinResizeListener, WinTerminal};
pub use timer::WinTimer;

pub fn tcp_raw_handle(stream: &std::net::TcpStream) -> super::types::RawHandle {
    use std::os::windows::io::AsRawSocket;
    stream.as_raw_socket() as super::types::RawHandle
}
