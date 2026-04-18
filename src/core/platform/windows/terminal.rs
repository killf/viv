use std::os::windows::raw::HANDLE;

use super::ffi;

pub struct WinTerminal {
    input_handle: HANDLE,
    output_handle: HANDLE,
    original_input_mode: u32,
    original_output_mode: u32,
    raw_mode_active: bool,
}

impl WinTerminal {
    pub fn new() -> crate::Result<Self> {
        let input_handle = unsafe { ffi::GetStdHandle(ffi::STD_INPUT_HANDLE) };
        let output_handle = unsafe { ffi::GetStdHandle(ffi::STD_OUTPUT_HANDLE) };

        // Detect non-interactive environments (pipe, CI, redirected stdin).
        // GetStdHandle returns a valid HANDLE even for pipes, but
        // GetConsoleMode fails with ERROR_INVALID_HANDLE (6) on non-console handles.
        let input_console_err;
        unsafe {
            let mut m = 0u32;
            ffi::GetConsoleMode(input_handle, &mut m);
            input_console_err = ffi::GetLastError();
        }
        if input_console_err == 6 {
            return Err(crate::Error::Terminal(
                "viv requires an interactive terminal. \
                stdin is not connected to a console (running in CI/non-interactive mode?). \
                If you want to run viv, please use a proper terminal/shell environment.".into(),
            ));
        }

        let mut input_mode = 0u32;
        let mut output_mode = 0u32;
        unsafe {
            ffi::GetConsoleMode(input_handle, &mut input_mode);
            ffi::GetConsoleMode(output_handle, &mut output_mode);
        }

        // Enable VT processing for ANSI escape sequences
        let new_output =
            output_mode | ffi::ENABLE_PROCESSED_OUTPUT | ffi::ENABLE_VIRTUAL_TERMINAL_PROCESSING;
        unsafe { ffi::SetConsoleMode(output_handle, new_output) };

        Ok(WinTerminal {
            input_handle,
            output_handle,
            original_input_mode: input_mode,
            original_output_mode: output_mode,
            raw_mode_active: false,
        })
    }

    pub fn enable_raw_mode(&mut self) -> crate::Result<()> {
        if self.raw_mode_active {
            return Ok(());
        }
        let raw = ffi::ENABLE_WINDOW_INPUT | ffi::ENABLE_VIRTUAL_TERMINAL_INPUT;
        if unsafe { ffi::SetConsoleMode(self.input_handle, raw) } == 0 {
            return Err(crate::Error::Io(std::io::Error::last_os_error()));
        }
        self.raw_mode_active = true;
        Ok(())
    }

    pub fn disable_raw_mode(&mut self) -> crate::Result<()> {
        if !self.raw_mode_active {
            return Ok(());
        }
        unsafe { ffi::SetConsoleMode(self.input_handle, self.original_input_mode) };
        self.raw_mode_active = false;
        Ok(())
    }

    pub fn size(&self) -> crate::Result<(u16, u16)> {
        let mut info = unsafe { std::mem::zeroed::<ffi::CONSOLE_SCREEN_BUFFER_INFO>() };
        if unsafe { ffi::GetConsoleScreenBufferInfo(self.output_handle, &mut info) } != 0 {
            let rows = (info.sr_window.bottom - info.sr_window.top + 1) as u16;
            let cols = (info.sr_window.right - info.sr_window.left + 1) as u16;
            Ok((rows, cols))
        } else {
            Ok((24, 80))
        }
    }

    pub fn input_handle(&self) -> HANDLE {
        self.input_handle
    }

    pub fn owns_input(&self) -> bool {
        true
    }

    pub fn read_input(&self, buf: &mut [u8]) -> crate::Result<usize> {
        let mut num_events = 0u32;
        unsafe { ffi::GetNumberOfConsoleInputEvents(self.input_handle, &mut num_events) };
        if num_events == 0 {
            return Err(crate::Error::Io(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "no input",
            )));
        }

        let mut records =
            vec![unsafe { std::mem::zeroed::<ffi::INPUT_RECORD>() }; num_events as usize];
        let mut read_count = 0u32;
        if unsafe {
            ffi::ReadConsoleInputW(
                self.input_handle,
                records.as_mut_ptr(),
                num_events,
                &mut read_count,
            )
        } == 0
        {
            return Err(crate::Error::Io(std::io::Error::last_os_error()));
        }

        let mut written = 0usize;
        for record in &records[..read_count as usize] {
            if record.event_type == ffi::KEY_EVENT {
                let key = unsafe { &*(record.event.as_ptr() as *const ffi::KEY_EVENT_RECORD) };
                if key.b_key_down != 0 && key.u_char != 0 {
                    if let Some(ch) = char::from_u32(key.u_char as u32) {
                        let mut utf8_buf = [0u8; 4];
                        let encoded = ch.encode_utf8(&mut utf8_buf);
                        let bytes = encoded.as_bytes();
                        if written + bytes.len() <= buf.len() {
                            buf[written..written + bytes.len()].copy_from_slice(bytes);
                            written += bytes.len();
                        }
                    }
                }
            }
        }
        Ok(written)
    }
}

impl Drop for WinTerminal {
    fn drop(&mut self) {
        self.disable_raw_mode().ok();
        unsafe { ffi::SetConsoleMode(self.output_handle, self.original_output_mode) };
    }
}

// SAFETY: Console HANDLEs are thread-safe in Windows.
unsafe impl Send for WinTerminal {}
unsafe impl Sync for WinTerminal {}

pub struct WinResizeListener {
    input_handle: HANDLE,
}

impl WinResizeListener {
    pub fn new() -> crate::Result<Self> {
        Ok(WinResizeListener {
            input_handle: unsafe { ffi::GetStdHandle(ffi::STD_INPUT_HANDLE) },
        })
    }

    pub fn handle(&self) -> HANDLE {
        self.input_handle
    }

    pub fn drain(&self) {
        // Nothing to drain on Windows — resize events come through ReadConsoleInput.
    }
}

// SAFETY: Console HANDLEs are thread-safe in Windows.
unsafe impl Send for WinResizeListener {}
unsafe impl Sync for WinResizeListener {}
