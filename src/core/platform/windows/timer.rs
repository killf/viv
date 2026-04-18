use std::os::windows::raw::HANDLE;
use std::time::Duration;

use super::ffi;

pub struct WinTimer {
    handle: HANDLE,
}

impl WinTimer {
    pub fn new(duration: Duration) -> crate::Result<Self> {
        let handle =
            unsafe { ffi::CreateWaitableTimerW(std::ptr::null_mut(), 1, std::ptr::null()) };
        if handle.is_null() {
            return Err(crate::Error::Io(std::io::Error::last_os_error()));
        }
        // Negative value = relative time in 100-nanosecond intervals
        let due_time = -(duration.as_nanos() as i64 / 100);
        if unsafe { ffi::SetWaitableTimer(handle, &due_time, 0, 0, 0, 0) } == 0 {
            unsafe { ffi::CloseHandle(handle) };
            return Err(crate::Error::Io(std::io::Error::last_os_error()));
        }
        Ok(WinTimer { handle })
    }

    pub fn handle(&self) -> HANDLE {
        self.handle
    }

    pub fn consume(&self) -> crate::Result<()> {
        Ok(())
    }
}

impl Drop for WinTimer {
    fn drop(&mut self) {
        unsafe { ffi::CloseHandle(self.handle) };
    }
}

// SAFETY: Waitable timer HANDLEs are thread-safe in Windows.
unsafe impl Send for WinTimer {}
