pub mod types;

#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

pub use types::RawHandle;

#[cfg(unix)]
pub type PlatformReactor = unix::EpollReactor;
#[cfg(windows)]
pub type PlatformReactor = windows::IocpReactor;

#[cfg(unix)]
pub type PlatformTimer = unix::UnixTimer;
#[cfg(windows)]
pub type PlatformTimer = windows::WinTimer;

#[cfg(unix)]
pub type PlatformNotifier = unix::PipeNotifier;
#[cfg(windows)]
pub type PlatformNotifier = windows::EventNotifier;

#[cfg(unix)]
pub type PlatformTerminal = unix::UnixTerminal;
#[cfg(windows)]
pub type PlatformTerminal = windows::WinTerminal;

#[cfg(unix)]
pub type PlatformResizeListener = unix::UnixResizeListener;
#[cfg(windows)]
pub type PlatformResizeListener = windows::WinResizeListener;

#[cfg(unix)]
pub use unix::shell_command;
#[cfg(windows)]
pub use windows::shell_command;

#[cfg(unix)]
pub use unix::tcp_raw_handle;
#[cfg(windows)]
pub use windows::tcp_raw_handle;
