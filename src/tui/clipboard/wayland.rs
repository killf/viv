//! Wayland clipboard support.
//!
//! Currently uses wl-copy subprocess (see mod.rs copy_via_wl_copy).
//! This file provides a placeholder for future native Wayland FFI.

/// Stub for future Wayland clipboard FFI implementation.
pub fn copy_via_wayland(_text: &str) -> Result<(), crate::error::Error> {
    Err(crate::error::Error::Terminal(
        "Native Wayland clipboard not yet implemented".into(),
    ))
}
