//! Zero-dependency async logging system.

use crate::core::sync::lock_or_recover;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

/// Log severity level.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Level {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
    Trace = 4,
}

impl Level {
    pub fn from_str(s: &str) -> Result<Self, ()> {
        match s {
            "ERROR" => Ok(Level::Error),
            "WARN" => Ok(Level::Warn),
            "INFO" => Ok(Level::Info),
            "DEBUG" => Ok(Level::Debug),
            "TRACE" => Ok(Level::Trace),
            _ => Err(()),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN ",
            Level::Info => "INFO ",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str().trim_end())
    }
}

impl std::str::FromStr for Level {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Level::from_str(s)
    }
}

/// Single log entry.
pub struct Record {
    time: String,
    level: Level,
    module: String,
    file: &'static str,
    line: u32,
    msg: String,
}

impl Record {
    pub fn new(level: Level, module: String, file: &'static str, line: u32, msg: String) -> Self {
        Record {
            time: timestamp(),
            level,
            module,
            file,
            line,
            msg,
        }
    }
}

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} [{}] {} ({}:{})\n",
            self.time, self.level, self.module, self.msg, self.file, self.line
        )
    }
}

/// Extract module name from a `file!()` path.
pub fn extract_module(path: &str) -> String {
    let path = path.strip_prefix("src/").unwrap_or(path);
    if let Some(pos) = path.rfind("/mod.rs") {
        path[..pos].replace('/', ".")
    } else if let Some(pos) = path.rfind(".rs") {
        path[..pos].replace('/', ".")
    } else {
        path.to_string()
    }
}

/// Format current time as RFC 3339.
fn timestamp() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let millis = dur.subsec_millis();
    let total_secs = secs as i64;
    let days = total_secs / 86400;
    let remainder = total_secs % 86400;
    let hours = remainder / 3600;
    let minutes = (remainder % 3600) / 60;
    let seconds = remainder % 60;
    let (year, month, day) = rata_die(days as i64);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year, month, day, hours, minutes, seconds, millis
    )
}

/// Convert days-since-1970 to (year, month, day) in UTC.
fn rata_die(days: i64) -> (i64, u8, u8) {
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as i64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    (y, m as u8, d as u8)
}

// ─── Background flush thread ──────────────────────────────────────────────

// Global registry of the active flusher JoinHandle.
// Used to ensure sequential init() calls wait for the previous flusher to exit.
static PREV_HANDLE: std::sync::Mutex<Option<thread::JoinHandle<()>>> = std::sync::Mutex::new(None);

// Stores (sender, shutdown_flag).
// - shutdown flag signals the thread to drain and exit.
// - Only ONE active logger at a time; re-init replaces the old one.
static FLUSH_STATE: std::sync::Mutex<Option<(mpsc::SyncSender<Arc<Record>>, Arc<AtomicBool>)>> =
    std::sync::Mutex::new(None);

const BUFFER_SIZE: usize = 256;
const FLUSH_TIMEOUT_MS: u64 = 1000;

fn file_size(path: &str) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024; // 10 MB

fn rotate_log(path: &str) -> io::Result<()> {
    for i in (1..5).rev() {
        let from = format!("{}.{}", path, i);
        let to = format!("{}.{}", path, i + 1);
        if Path::new(&from).exists() {
            fs::rename(&from, &to)?;
        }
    }
    if Path::new(path).exists() {
        fs::rename(path, format!("{}.1", path))?;
    }
    Ok(())
}

fn spawn_flusher(path: String, level: Level) -> Option<thread::JoinHandle<()>> {
    let (tx, rx) = mpsc::sync_channel::<Arc<Record>>(1024);
    let shutdown = Arc::new(AtomicBool::new(false));

    // Take the previous handle from the global registry.
    let prev = { lock_or_recover(&PREV_HANDLE).take() };
    {
        let mut guard = lock_or_recover(&FLUSH_STATE);
        if let Some((_, old_shutdown)) = guard.take() {
            old_shutdown.store(true, Ordering::SeqCst);
        }
        *guard = Some((tx, shutdown.clone()));
    }

    let handle = thread::spawn(move || {
        let file = match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("[log] failed to open {}: {}", path, e);
                return;
            }
        };
        let mut writer = BufWriter::new(file);
        let mut buf: Vec<Arc<Record>> = Vec::with_capacity(BUFFER_SIZE);
        let mut since_flush = std::time::Instant::now();

        loop {
            // Check shutdown first
            if shutdown.load(Ordering::SeqCst) {
                for rec in buf.drain(..) {
                    let _ = writer.write_all(rec.to_string().as_bytes());
                }
                let _ = writer.flush();
                return;
            }

            match rx.recv_timeout(std::time::Duration::from_millis(FLUSH_TIMEOUT_MS)) {
                Ok(rec) => {
                    if rec.level <= level {
                        buf.push(rec);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    for rec in buf.drain(..) {
                        let _ = writer.write_all(rec.to_string().as_bytes());
                    }
                    let _ = writer.flush();
                    return;
                }
            }

            if buf.len() >= BUFFER_SIZE
                || since_flush.elapsed().as_millis() as u64 >= FLUSH_TIMEOUT_MS
            {
                for rec in buf.drain(..) {
                    let _ = writer.write_all(rec.to_string().as_bytes());
                }
                let _ = writer.flush();
                if file_size(&path) > MAX_LOG_SIZE {
                    let _ = rotate_log(&path);
                }
                since_flush = std::time::Instant::now();
            }
        }
    });

    // Register the new handle so the NEXT init() can wait for it.
    *lock_or_recover(&PREV_HANDLE) = Some(handle);

    // Return previous handle so init() can wait for it.
    prev
}

/// Initialize the global logger. Spawns the background flush thread.
/// Subsequent calls replace the previous logger (old thread drains and exits).
pub fn init(path: &str, level: Level) -> io::Result<()> {
    // Wait for the previous flusher to fully exit before returning.
    // This ensures the new flusher has opened its file before any logging.
    if let Some(prev) = spawn_flusher(path.to_string(), level) {
        let _ = prev.join();
    }
    Ok(())
}

/// Core logging function. Called by macros.
pub fn log(level: Level, module: String, file: &'static str, line: u32, msg: String) {
    let guard = lock_or_recover(&FLUSH_STATE);
    if let Some((tx, shutdown)) = guard.as_ref() {
        if shutdown.load(Ordering::SeqCst) {
            return;
        }
        let rec = Arc::new(Record::new(level, module, file, line, msg));
        let _ = tx.try_send(rec);
    }
}

mod macros;
