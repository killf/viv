//! Cross-platform clipboard integration.
//!
//! Detects the current platform and copies text to the system clipboard.

use crate::error::Error;

#[cfg(target_os = "linux")]
mod x11;

#[cfg(target_os = "linux")]
mod wayland;

/// Detected clipboard backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardBackend {
    /// Linux X11 (via libX11 FFI)
    X11,
    /// Linux Wayland (via wl-copy subprocess)
    Wayland,
    /// macOS (via AppKit FFI)
    MacOS,
    /// Windows (via Win32 FFI)
    Windows,
    /// No supported clipboard backend
    Unsupported,
}

impl ClipboardBackend {
    /// Detect the available clipboard backend for this platform.
    pub fn detect() -> Self {
        #[cfg(target_os = "linux")]
        {
            if std::env::var("WAYLAND_DISPLAY").is_ok() {
                return ClipboardBackend::Wayland;
            }
            if std::env::var("DISPLAY").is_ok() {
                return ClipboardBackend::X11;
            }
            return ClipboardBackend::Unsupported;
        }

        #[cfg(target_os = "macos")]
        {
            return ClipboardBackend::MacOS;
        }

        #[cfg(target_os = "windows")]
        {
            return ClipboardBackend::Windows;
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            return ClipboardBackend::Unsupported;
        }
    }
}

/// Copy text to the system clipboard.
/// Returns Ok(()) on success, or an error with a description.
pub fn copy(text: &str) -> Result<(), Error> {
    let backend = ClipboardBackend::detect();

    match backend {
        ClipboardBackend::X11 => {
            x11::copy_via_x11(text)
        }
        ClipboardBackend::Wayland => {
            copy_via_wl_copy(text)
        }
        ClipboardBackend::MacOS => {
            copy_via_pbcopy(text)
        }
        ClipboardBackend::Windows => {
            copy_via_clip(text)
        }
        ClipboardBackend::Unsupported => {
            // Fallback: print to stderr
            eprintln!("Clipboard not supported on this platform");
            Err(Error::Terminal("Clipboard not supported".into()))
        }
    }
}

// ── Wayland: use wl-copy if available ────────────────────────────────────────

#[cfg(target_os = "linux")]
fn copy_via_wl_copy(text: &str) -> Result<(), Error> {
    use std::process::Command;

    let child = Command::new("wl-copy")
        .stdin(std::process::Stdio::piped())
        .spawn();

    match child {
        Ok(mut child) => {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                if stdin.write_all(text.as_bytes()).is_ok() {
                    drop(stdin);
                    if child.wait().map(|s| s.success()).unwrap_or(false) {
                        return Ok(());
                    }
                }
            }
            Err(Error::Terminal("wl-copy failed".into()))
        }
        Err(_) => Err(Error::Terminal("wl-copy not found".into())),
    }
}

// ── macOS: use pbcopy ────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn copy_via_pbcopy(text: &str) -> Result<(), Error> {
    use std::io::Write;
    use std::process::Command;

    let mut child = Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| Error::Terminal(format!("pbcopy spawn failed: {}", e)))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| Error::Terminal(format!("pbcopy write failed: {}", e)))?;
        drop(stdin);
    }

    child
        .wait()
        .map_err(|e| Error::Terminal(format!("pbcopy wait failed: {}", e)))?
        .success()
        .then_some(())
        .ok_or_else(|| Error::Terminal("pbcopy failed".into()))
}

// ── Windows: use clip.exe ─────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn copy_via_clip(text: &str) -> Result<(), Error> {
    use std::io::Write;
    use std::process::Command;

    let mut child = Command::new("clip.exe")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| Error::Terminal(format!("clip.exe spawn failed: {}", e)))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| Error::Terminal(format!("clip.exe write failed: {}", e)))?;
        drop(stdin);
    }

    child
        .wait()
        .map_err(|e| Error::Terminal(format!("clip.exe wait failed: {}", e)))?
        .success()
        .then_some(())
        .ok_or_else(|| Error::Terminal("clip.exe failed".into()))
}

// ── Unsupported platform stubs ───────────────────────────────────────────────

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn copy_via_wl_copy(_text: &str) -> Result<(), Error> {
    Err(Error::Terminal("Clipboard not supported".into()))
}

#[cfg(not(target_os = "macos"))]
fn copy_via_pbcopy(_text: &str) -> Result<(), Error> {
    Err(Error::Terminal("pbcopy not available".into()))
}

#[cfg(not(target_os = "windows"))]
fn copy_via_clip(_text: &str) -> Result<(), Error> {
    Err(Error::Terminal("clip.exe not available".into()))
}
