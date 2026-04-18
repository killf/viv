#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use std::os::windows::raw::HANDLE;

pub const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;
pub const NULL_HANDLE: HANDLE = std::ptr::null_mut();

// Standard handles
pub const STD_INPUT_HANDLE: u32 = 0xFFFFFFF6;
pub const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5;

// Console mode flags
pub const ENABLE_PROCESSED_INPUT: u32 = 0x0001;
pub const ENABLE_LINE_INPUT: u32 = 0x0002;
pub const ENABLE_ECHO_INPUT: u32 = 0x0004;
pub const ENABLE_WINDOW_INPUT: u32 = 0x0008;
pub const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;
pub const ENABLE_PROCESSED_OUTPUT: u32 = 0x0001;
pub const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

// Wait constants
pub const WAIT_OBJECT_0: u32 = 0;
pub const INFINITE: u32 = 0xFFFFFFFF;

// Input record event types
pub const KEY_EVENT: u16 = 0x0001;
pub const WINDOW_BUFFER_SIZE_EVENT: u16 = 0x0004;

#[repr(C)]
pub struct OVERLAPPED {
    pub internal: usize,
    pub internal_high: usize,
    pub offset: u32,
    pub offset_high: u32,
    pub h_event: HANDLE,
}

#[repr(C)]
pub struct COORD {
    pub x: i16,
    pub y: i16,
}

#[repr(C)]
pub struct SMALL_RECT {
    pub left: i16,
    pub top: i16,
    pub right: i16,
    pub bottom: i16,
}

#[repr(C)]
pub struct CONSOLE_SCREEN_BUFFER_INFO {
    pub dw_size: COORD,
    pub dw_cursor_position: COORD,
    pub w_attributes: u16,
    pub sr_window: SMALL_RECT,
    pub dw_maximum_window_size: COORD,
}

#[repr(C)]
pub struct KEY_EVENT_RECORD {
    pub b_key_down: i32,
    pub w_repeat_count: u16,
    pub w_virtual_key_code: u16,
    pub w_virtual_scan_code: u16,
    pub u_char: u16,
    pub dw_control_key_state: u32,
}

#[repr(C)]
pub struct WINDOW_BUFFER_SIZE_RECORD {
    pub dw_size: COORD,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct INPUT_RECORD {
    pub event_type: u16,
    pub _padding: u16,
    pub event: [u8; 16],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn GetStdHandle(nStdHandle: u32) -> HANDLE;
    pub fn GetConsoleMode(hConsoleHandle: HANDLE, lpMode: *mut u32) -> i32;
    pub fn SetConsoleMode(hConsoleHandle: HANDLE, dwMode: u32) -> i32;
    pub fn GetConsoleScreenBufferInfo(
        hConsoleHandle: HANDLE,
        lpConsoleScreenBufferInfo: *mut CONSOLE_SCREEN_BUFFER_INFO,
    ) -> i32;
    pub fn ReadConsoleInputW(
        hConsoleInput: HANDLE,
        lpBuffer: *mut INPUT_RECORD,
        nLength: u32,
        lpNumberOfEventsRead: *mut u32,
    ) -> i32;
    pub fn GetNumberOfConsoleInputEvents(hConsoleInput: HANDLE, lpcNumberOfEvents: *mut u32)
    -> i32;

    pub fn CreateIoCompletionPort(
        FileHandle: HANDLE,
        ExistingCompletionPort: HANDLE,
        CompletionKey: usize,
        NumberOfConcurrentThreads: u32,
    ) -> HANDLE;
    pub fn GetQueuedCompletionStatus(
        CompletionPort: HANDLE,
        lpNumberOfBytesTransferred: *mut u32,
        lpCompletionKey: *mut usize,
        lpOverlapped: *mut *mut OVERLAPPED,
        dwMilliseconds: u32,
    ) -> i32;
    pub fn PostQueuedCompletionStatus(
        CompletionPort: HANDLE,
        dwNumberOfBytesTransferred: u32,
        dwCompletionKey: usize,
        lpOverlapped: *mut OVERLAPPED,
    ) -> i32;

    pub fn CreateWaitableTimerW(
        lpTimerAttributes: *mut u8,
        bManualReset: i32,
        lpTimerName: *const u16,
    ) -> HANDLE;
    pub fn SetWaitableTimer(
        hTimer: HANDLE,
        lpDueTime: *const i64,
        lPeriod: i32,
        pfnCompletionRoutine: usize,
        lpArgToCompletionRoutine: usize,
        fResume: i32,
    ) -> i32;

    pub fn CreateEventW(
        lpEventAttributes: *mut u8,
        bManualReset: i32,
        bInitialState: i32,
        lpName: *const u16,
    ) -> HANDLE;
    pub fn SetEvent(hEvent: HANDLE) -> i32;
    pub fn ResetEvent(hEvent: HANDLE) -> i32;

    pub fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: u32) -> u32;
    pub fn WaitForMultipleObjects(
        nCount: u32,
        lpHandles: *const HANDLE,
        bWaitAll: i32,
        dwMilliseconds: u32,
    ) -> u32;

    pub fn CloseHandle(hObject: HANDLE) -> i32;
    pub fn GetLastError() -> u32;
}
