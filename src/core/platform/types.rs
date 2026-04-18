/// Cross-platform handle type.
#[cfg(unix)]
pub type RawHandle = std::os::unix::io::RawFd;

#[cfg(windows)]
pub type RawHandle = std::os::windows::raw::HANDLE;

#[cfg(unix)]
pub const INVALID_HANDLE: RawHandle = -1;

#[cfg(windows)]
pub const INVALID_HANDLE: RawHandle = -1isize as RawHandle;
