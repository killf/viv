//! X11 clipboard integration via FFI.
//!
//! Implements the ICCCM clipboard protocol:
//! 1. Open X11 display
//! 2. Set selection owner to root window
//! 3. Change property on root window with clipboard data

use crate::error::Error;
use std::ffi::CString;
use std::ptr;

// ── X11 constants ─────────────────────────────────────────────────────────────

const CURRENT_TIME: u64 = 0;
const UTF8_STRING: &[u8] = b"UTF8_STRING\0";

// ── X11 FFI declarations ───────────────────────────────────────────────────

#[derive(Clone, Copy)]
#[repr(C)]
struct X11Atom(u32);

#[derive(Clone, Copy)]
#[repr(C)]
struct XWindow(u32);

#[repr(C)]
struct _XDisplay {
    _private: [u8; 0],
}
type XDisplay = _XDisplay;

#[link(name = "X11")]
unsafe extern "C" {
    fn XOpenDisplay(display_name: *const i8) -> *mut XDisplay;
    fn XCloseDisplay(dpy: *mut XDisplay) -> i32;
    fn XInternAtom(dpy: *mut XDisplay, name: *const i8, only_if_exists: i32) -> X11Atom;
    fn XSetSelectionOwner(
        dpy: *mut XDisplay,
        selection: X11Atom,
        owner: XWindow,
        time: u64,
    ) -> i32;
    fn XChangeProperty(
        dpy: *mut XDisplay,
        window: XWindow,
        property: X11Atom,
        typ: X11Atom,
        format: i32,
        mode: i32,
        data: *const u8,
        nelements: i32,
    );
    fn XFlush(dpy: *mut XDisplay) -> i32;
    fn XDefaultRootWindow(dpy: *mut XDisplay) -> XWindow;
}

// ── Helper: get atom, returning None on failure ──────────────────────────────

fn intern_atom(dpy: *mut XDisplay, name: &[u8]) -> Option<X11Atom> {
    let name_cstr = CString::from_vec_with_nul(name.to_vec()).ok()?;
    let atom = unsafe { XInternAtom(dpy, name_cstr.as_ptr(), 0) };
    if atom.0 == 0 {
        None
    } else {
        Some(atom)
    }
}

// ── Copy via X11 ─────────────────────────────────────────────────────────────

/// Copy text to X11 clipboard using ICCCM protocol.
pub fn copy_via_x11(text: &str) -> Result<(), Error> {
    // Open display
    let dpy = unsafe { XOpenDisplay(ptr::null()) };
    if dpy.is_null() {
        return Err(Error::Terminal("XOpenDisplay failed".into()));
    }
    let dpy = DpyHandle(dpy);

    // Get atoms
    let clipboard_atom = intern_atom(dpy.0, UTF8_STRING)
        .ok_or_else(|| Error::Terminal("Failed to intern UTF8_STRING atom".into()))?;

    // Get root window
    let root = unsafe { XDefaultRootWindow(dpy.0) };

    // Set ourselves as the selection owner
    let result = unsafe {
        XSetSelectionOwner(dpy.0, clipboard_atom, root, CURRENT_TIME)
    };
    if result == 0 {
        return Err(Error::Terminal("XSetSelectionOwner failed".into()));
    }

    // Change property on root window with clipboard data
    let text_bytes = text.as_bytes();
    let nelements = text_bytes.len() as i32;

    unsafe {
        XChangeProperty(
            dpy.0,
            root,
            clipboard_atom,
            clipboard_atom,
            8, // 8 bits per byte
            0, // PropModeReplace
            text_bytes.as_ptr(),
            nelements,
        );
        XFlush(dpy.0);
    }

    Ok(())
}

// RAII wrapper for Display
struct DpyHandle(*mut XDisplay);

impl Drop for DpyHandle {
    fn drop(&mut self) {
        unsafe { XCloseDisplay(self.0) };
    }
}
