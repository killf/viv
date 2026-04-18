use std::collections::HashMap;
use std::os::windows::raw::HANDLE;
use std::task::Waker;
use std::time::Duration;

use super::ffi;

pub struct IocpReactor {
    port: HANDLE,
    wakers: HashMap<u64, Waker>,
    next_token: u64,
}

impl IocpReactor {
    pub fn new() -> crate::Result<Self> {
        let port = unsafe {
            ffi::CreateIoCompletionPort(ffi::INVALID_HANDLE_VALUE, ffi::NULL_HANDLE, 0, 1)
        };
        if port.is_null() {
            return Err(crate::Error::Io(std::io::Error::last_os_error()));
        }
        Ok(IocpReactor {
            port,
            wakers: HashMap::new(),
            next_token: 1,
        })
    }

    pub fn register_read(&mut self, _handle: HANDLE, waker: Waker) -> crate::Result<u64> {
        let token = self.next_token;
        self.next_token += 1;
        self.wakers.insert(token, waker);
        unsafe {
            ffi::PostQueuedCompletionStatus(self.port, 0, token as usize, std::ptr::null_mut())
        };
        Ok(token)
    }

    pub fn register_write(&mut self, _handle: HANDLE, waker: Waker) -> crate::Result<u64> {
        let token = self.next_token;
        self.next_token += 1;
        self.wakers.insert(token, waker);
        unsafe {
            ffi::PostQueuedCompletionStatus(self.port, 0, token as usize, std::ptr::null_mut())
        };
        Ok(token)
    }

    pub fn deregister(&mut self, token: u64) -> crate::Result<()> {
        self.wakers.remove(&token);
        Ok(())
    }

    pub fn poll(&mut self, timeout: Duration) -> crate::Result<usize> {
        let ms = timeout.as_millis().min(u32::MAX as u128) as u32;
        let mut bytes = 0u32;
        let mut key = 0usize;
        let mut overlapped: *mut ffi::OVERLAPPED = std::ptr::null_mut();
        let ret = unsafe {
            ffi::GetQueuedCompletionStatus(self.port, &mut bytes, &mut key, &mut overlapped, ms)
        };
        if ret != 0 {
            let token = key as u64;
            if let Some(waker) = self.wakers.remove(&token) {
                waker.wake();
            }
            Ok(1)
        } else {
            Ok(0)
        }
    }
}

impl Drop for IocpReactor {
    fn drop(&mut self) {
        unsafe { ffi::CloseHandle(self.port) };
    }
}

// SAFETY: HANDLE (IOCP port) is thread-safe — Windows guarantees
// completion port handles can be used from any thread.
unsafe impl Send for IocpReactor {}
unsafe impl Sync for IocpReactor {}
