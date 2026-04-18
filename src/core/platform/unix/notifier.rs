use crate::core::platform::types::RawHandle;

unsafe extern "C" {
    fn pipe(pipefd: *mut [i32; 2]) -> i32;
    fn close(fd: i32) -> i32;
    fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
}

const F_GETFL: i32 = 3;
const F_SETFL: i32 = 4;
const O_NONBLOCK: i32 = 0o4000;

fn set_nonblocking(fd: i32) {
    unsafe {
        let flags = fcntl(fd, F_GETFL);
        fcntl(fd, F_SETFL, flags | O_NONBLOCK);
    }
}

pub struct PipeNotifier {
    read_fd: i32,
    write_fd: i32,
}

impl PipeNotifier {
    pub fn new() -> crate::Result<Self> {
        let mut fds = [0i32; 2];
        let ret = unsafe { pipe(&mut fds) };
        if ret != 0 {
            return Err(crate::Error::Io(std::io::Error::last_os_error()));
        }
        set_nonblocking(fds[0]);
        set_nonblocking(fds[1]);
        Ok(PipeNotifier {
            read_fd: fds[0],
            write_fd: fds[1],
        })
    }

    pub fn handle(&self) -> RawHandle {
        self.read_fd
    }

    pub fn notify(&self) -> crate::Result<()> {
        let byte: u8 = 1;
        unsafe { write(self.write_fd, &byte, 1) };
        Ok(())
    }

    pub fn drain(&self) -> crate::Result<()> {
        let mut buf = [0u8; 64];
        loop {
            let n = unsafe { read(self.read_fd, buf.as_mut_ptr(), buf.len()) };
            if n <= 0 {
                break;
            }
        }
        Ok(())
    }
}

impl Drop for PipeNotifier {
    fn drop(&mut self) {
        unsafe {
            close(self.read_fd);
            close(self.write_fd);
        }
    }
}
