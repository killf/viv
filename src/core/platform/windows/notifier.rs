use std::os::windows::raw::HANDLE;

use super::ffi;

pub struct EventNotifier {
    event: HANDLE,
}

impl EventNotifier {
    pub fn new() -> crate::Result<Self> {
        let event = unsafe { ffi::CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };
        if event.is_null() {
            return Err(crate::Error::Io(std::io::Error::last_os_error()));
        }
        Ok(EventNotifier { event })
    }

    pub fn handle(&self) -> HANDLE {
        self.event
    }

    pub fn notify(&self) -> crate::Result<()> {
        if unsafe { ffi::SetEvent(self.event) } == 0 {
            return Err(crate::Error::Io(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    pub fn drain(&self) -> crate::Result<()> {
        unsafe { ffi::ResetEvent(self.event) };
        Ok(())
    }
}

impl Drop for EventNotifier {
    fn drop(&mut self) {
        unsafe { ffi::CloseHandle(self.event) };
    }
}
