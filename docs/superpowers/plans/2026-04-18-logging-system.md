# Zero-Dependency Logging System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a zero-dependency async logging system with five severity levels, automatic module labeling, buffered writes, and log rotation.

**Architecture:** Global singleton logger with a dedicated background flush thread. Call sites use `mpsc::Sender` to enqueue records without blocking; the flush thread batches writes to file (256 records or 1s timeout). Module labels extracted from `file!()` macro at compile time.

**Tech Stack:** Rust stdlib only (`std::sync::mpsc`, `std::sync::OnceLock`, `std::fs`, `std::io::BufWriter`, `std::time::SystemTime`)

---

## File Structure

```
src/log/
├── mod.rs    # Level, Record, Logger, init(), module_name! macro, log! fn
└── macros.rs # info!, debug!, warn!, error!, trace! macros
tests/log/
└── mod.rs    # Unit tests (module extraction, level filtering, formatting)
src/lib.rs    # Add: pub mod log;
```

---

## Task 1: Create `src/log/mod.rs`

**Files:**
- Create: `src/log/mod.rs`
- Test: `tests/log/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/log/mod.rs`:

```rust
use viv::log::{Level, Record, module_name};

// ─── module_name! macro ───────────────────────────────────────

#[test]
fn module_name_from_src_path() {
    // Simulate: src/agent/mod.rs → "agent"
    let path = "src/agent/mod.rs";
    assert_eq!(extract_module_for_test(path), "agent");
}

#[test]
fn module_name_from_src_file() {
    // src/llm.rs → "llm"
    let path = "src/llm.rs";
    assert_eq!(extract_module_for_test(path), "llm");
}

#[test]
fn module_name_from_deep_path() {
    // src/core/runtime/mod.rs → "core/runtime"
    let path = "src/core/runtime/mod.rs";
    assert_eq!(extract_module_for_test(path), "core/runtime");
}

#[test]
fn module_name_strips_src_prefix() {
    // just/llm.rs (no src/) → "just/llm" stripped of .rs suffix
    let path = "just/llm.rs";
    assert_eq!(extract_module_for_test(path), "just/llm");
}

// ─── Level ───────────────────────────────────────────────────

#[test]
fn level_ordering() {
    assert!(Level::Error < Level::Warn);
    assert!(Level::Warn  < Level::Info);
    assert!(Level::Info  < Level::Debug);
    assert!(Level::Debug < Level::Trace);
}

#[test]
fn level_from_str() {
    assert_eq!("ERROR".parse::<Level>(), Ok(Level::Error));
    assert_eq!("WARN".parse::<Level>(),  Ok(Level::Warn));
    assert_eq!("INFO".parse::<Level>(),  Ok(Level::Info));
    assert_eq!("DEBUG".parse::<Level>(), Ok(Level::Debug));
    assert_eq!("TRACE".parse::<Level>(), Ok(Level::Trace));
    assert_eq!("BAD".parse::<Level>(),   Err(()));
}

// ─── Record formatting ───────────────────────────────────────

#[test]
fn record_format_contains_time() {
    let rec = Record::new(Level::Info, "test", "src/test.rs", 42, "hello world");
    let s = rec.to_string();
    assert!(s.contains("hello world"));
    assert!(s.contains("INFO"));
    assert!(s.contains("[test]"));
    assert!(s.contains("src/test.rs:42"));
}

#[test]
fn record_format_time_is_rfc3339() {
    let rec = Record::new(Level::Info, "test", "src/test.rs", 42, "msg");
    let s = rec.to_string();
    // RFC 3339: 2026-04-18T10:30:00.000Z
    assert!(s.starts_with("20")); // year starts with 20
    assert!(s.contains("T"));
    assert!(s.ends_with("Z\n"));
}
```

Helper for tests (put in `tests/log/mod.rs` top):

```rust
// Duplicates the private logic so tests can call it directly
fn extract_module_for_test(path: &str) -> String {
    let path = path.strip_prefix("src/").unwrap_or(path);
    if let Some(pos) = path.rfind("/mod.rs") {
        path[..pos].replace('/', ".")
    } else if let Some(pos) = path.rfind(".rs") {
        path[..pos].replace('/', ".")
    } else {
        path.to_string()
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test log_test 2>&1` (file doesn't exist yet)
Expected: error: no such test file

- [ ] **Step 3: Create `src/log/mod.rs`**

```rust
//! Zero-dependency async logging system.
//!
//! Usage:
//!     log::init("viv.log", Level::Info)?;
//!     info!("agent started");
//!     debug!("tool result: {:?}", result);

use std::cell::RefCell;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::sync::mpsc;
use std::sync::OnceLock;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

/// Log severity level.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Level {
    Error = 0,
    Warn  = 1,
    Info  = 2,
    Debug = 3,
    Trace = 4,
}

impl Level {
    pub fn from_str(s: &str) -> Result<Self, ()> {
        match s {
            "ERROR" => Ok(Level::Error),
            "WARN"  => Ok(Level::Warn),
            "INFO"  => Ok(Level::Info),
            "DEBUG" => Ok(Level::Debug),
            "TRACE" => Ok(Level::Trace),
            _       => Err(()),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn  => "WARN ",
            Level::Info  => "INFO ",
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

/// Single log entry.
pub struct Record {
    /// RFC 3339 timestamp string.
    time:   String,
    level:  Level,
    module: &'static str,
    file:   &'static str,
    line:   u32,
    msg:    String,
}

impl Record {
    pub fn new(level: Level, module: &'static str, file: &'static str, line: u32, msg: String) -> Self {
        Record { time: timestamp(), level, module, file, line, msg }
    }
}

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} [{}] {} ({}:{})\n",
            self.time,
            self.level,
            self.module,
            self.msg,
            self.file,
            self.line
        )
    }
}

/// Extract module name from a `file!()` path.
fn extract_module(path: &str) -> String {
    let path = path.strip_prefix("src/").unwrap_or(path);
    if let Some(pos) = path.rfind("/mod.rs") {
        path[..pos].replace('/', ".")
    } else if let Some(pos) = path.rfind(".rs") {
        path[..pos].replace('/', ".")
    } else {
        path.to_string()
    }
}

/// Runtime `module_name!` equivalent — extracts from `file!()` at the call site.
#[macro_export]
macro_rules! module_name {
    () => {
        $crate::log::extract_module(file!())
    };
}

/// Format current time as RFC 3339.
fn timestamp() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let millis = dur.subsec_millis();
    // Convert to broken-down UTC without chrono
    let total_secs = secs as i64;
    let days = total_secs / 86400;
    let remainder = total_secs % 86400;
    let hours = remainder / 3600;
    let minutes = (remainder % 3600) / 60;
    let seconds = remainder % 60;

    // Rata Die: days since 1970-01-01
    // We can use a simple formula since we only need years
    let (year, month, day) = rata_die(days as i64);

    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
            year, month, day, hours, minutes, seconds, millis)
}

/// Convert days-since-1970 to (year, month, day) in UTC.
/// Uses Zeller-like algorithm, valid for positive years.
fn rata_die(days: i64) -> (i64, u8, u8) {
    // Adapted from Howard Hinnant's civil_from_days
    let days = days;
    let era = if days >= 0 { days / 146097 } else { (days - 146096) / 146097 };
    let doe = days - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = y + (m <= 2) as i64; // Jan/Feb are months 13/14 of previous year
    (y, m as u8, d as u8)
}

// ─── Logger ────────────────────────────────────────────────────────────────

static LOGGER: OnceLock<Mutex<Logger>> = OnceLock::new();

struct Mutex<T>(std::sync::Mutex<T>);

impl<T> Mutex<T> {
    fn new(val: T) -> Self { Mutex(std::sync::Mutex::new(val)) }
    fn lock(&self) -> std::sync::MutexGuard<'_, T> { self.0.lock().unwrap() }
}

/// Initialize the global logger. Spawns the background flush thread.
/// Must be called once before any logging call.
pub fn init(path: &str, level: Level) -> io::Result<()> {
    let logger = Logger::new(Path::new(path), level)?;
    // Install into global
    let _ = LOGGER.set(Mutex::new(logger));
    Ok(())
}

/// Core logging function. Called by macros.
pub fn log(level: Level, module: &'static str, file: &'static str, line: u32, msg: String) {
    if let Some(lg) = LOGGER.get() {
        let rec = Record::new(level, module, file, line, msg);
        lg.lock().log(&rec);
    }
}
```

- [ ] **Step 4: Add `pub mod log` to `src/lib.rs`**

Modify `src/lib.rs` — add after line 10 (`pub mod tui;`):

```rust
pub mod log;
```

- [ ] **Step 5: Create `tests/log_test.rs` with tests above**

```rust
//! Unit tests for the log module.

#![allow(unused)]

// ─── module_name! macro ───────────────────────────────────────

#[test]
fn module_name_from_src_path() {
    let path = "src/agent/mod.rs";
    assert_eq!(extract_module(path), "agent");
}

#[test]
fn module_name_from_src_file() {
    let path = "src/llm.rs";
    assert_eq!(extract_module(path), "llm");
}

#[test]
fn module_name_from_deep_path() {
    let path = "src/core/runtime/mod.rs";
    assert_eq!(extract_module(path), "core.runtime");
}

// ─── Level ───────────────────────────────────────────────────

#[test]
fn level_ordering() {
    assert!(viv::log::Level::Error < viv::log::Level::Warn);
    assert!(viv::log::Level::Warn  < viv::log::Level::Info);
    assert!(viv::log::Level::Info  < viv::log::Level::Debug);
    assert!(viv::log::Level::Debug < viv::log::Level::Trace);
}

#[test]
fn level_from_str() {
    assert_eq!("ERROR".parse(), Ok(viv::log::Level::Error));
    assert_eq!("WARN".parse(),  Ok(viv::log::Level::Warn));
    assert_eq!("INFO".parse(),  Ok(viv::log::Level::Info));
    assert_eq!("DEBUG".parse(), Ok(viv::log::Level::Debug));
    assert_eq!("TRACE".parse(), Ok(viv::log::Level::Trace));
    assert!("BAD".parse::<viv::log::Level>().is_err());
}

// ─── Record formatting ───────────────────────────────────────

#[test]
fn record_format_contains_time() {
    use viv::log::{Level, Record};
    let rec = Record::new(Level::Info, "test", "src/test.rs", 42, "hello world".into());
    let s = rec.to_string();
    assert!(s.contains("hello world"));
    assert!(s.contains("INFO"));
    assert!(s.contains("[test]"));
    assert!(s.contains("src/test.rs:42"));
}

#[test]
fn record_format_time_is_rfc3339() {
    use viv::log::{Level, Record};
    let rec = Record::new(Level::Info, "test", "src/test.rs", 42, "msg".into());
    let s = rec.to_string();
    // RFC 3339: 2026-04-18T10:30:00.000Z
    assert!(s.starts_with("20")); // year starts with 20
    assert!(s.contains('T'));
    assert!(s.ends_with("Z\n") || s.ends_with("Z\r\n"));
}

// ─── Internal helpers (duplicated so tests are self-contained) ─────────────

fn extract_module(path: &str) -> String {
    let path = path.strip_prefix("src/").unwrap_or(path);
    if let Some(pos) = path.rfind("/mod.rs") {
        path[..pos].replace('/', ".")
    } else if let Some(pos) = path.rfind(".rs") {
        path[..pos].replace('/', ".")
    } else {
        path.to_string()
    }
}
```

Note: `Level::from_str` needs to implement `std::str::FromStr`. Add to `src/log/mod.rs`:

```rust
impl std::str::FromStr for Level {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> { Level::from_str(s) }
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --test log_test 2>&1`
Expected: all 7 tests PASS

- [ ] **Step 7: Commit**

```bash
git add src/log/mod.rs tests/log_test.rs src/lib.rs
git commit -m "$(cat <<'EOF'
feat(log): add zero-dependency logging system

Adds Level, Record, Logger, and init() with RFC 3339 timestamps.
Module labels extracted from file!() macro. Buffered writes.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Create `src/log/macros.rs` and async flush thread

**Files:**
- Modify: `src/log/mod.rs` — replace direct-write Logger with channel-based flush thread + rotation
- Create: `src/log/macros.rs`

Note: Task 1's `struct Logger { file, level }` + direct write is the initial scaffold. Task 2 replaces it with the channel-based background flush approach entirely.

- [ ] **Step 1: Update `src/log/mod.rs` — add background flush thread and rotation**

Replace the current Logger/init implementation in `src/log/mod.rs` with the following.

Add new static and types before `init()`:

```rust
// ─── Background flush thread ──────────────────────────────────────────────

static FLUSH_TX: OnceLock<mpsc::Sender<Arc<Record>>> = OnceLock::new();

const BUFFER_SIZE: usize = 256;
const FLUSH_TIMEOUT_MS: u64 = 1000;

/// Start the background flush thread. Records are batched and written
/// to `path` asynchronously. Called automatically by `init()`.
fn spawn_flusher(path: String, level: Level) {
    let (tx, rx) = mpsc::channel();
    let _ = FLUSH_TX.set(tx);

    thread::spawn(move || {
        let file = match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
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
            // Drain the channel with a timeout
            match rx.recv_timeout(std::time::Duration::from_millis(FLUSH_TIMEOUT_MS)) {
                Ok(rec) => {
                    if rec.level <= level {
                        buf.push(rec);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // No more senders; flush remaining and exit
                    for rec in buf.drain(..) {
                        let _ = writer.write_all(rec.to_string().as_bytes());
                    }
                    let _ = writer.flush();
                    return;
                }
            }

            // Flush if buffer full or timeout elapsed
            if buf.len() >= BUFFER_SIZE || (since_flush.elapsed().as_millis() as u64) >= FLUSH_TIMEOUT_MS {
                for rec in buf.drain(..) {
                    let _ = writer.write_all(rec.to_string().as_bytes());
                }
                let _ = writer.flush();
                since_flush = std::time::Instant::now();
            }
        }
    });
}
```

Update `init()`:

```rust
pub fn init(path: &str, level: Level) -> io::Result<()> {
    // Touch the file to ensure it exists and is writable
    {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let mut bw = BufWriter::new(file);
        let _ = bw.flush();
    }
    spawn_flusher(path.to_string(), level);
    Ok(())
}
```

Update the `log()` function to use the channel instead of direct write:

```rust
pub fn log(level: Level, module: &'static str, file: &'static str, line: u32, msg: String) {
    if let Some(tx) = FLUSH_TX.get() {
        let rec = Arc::new(Record::new(level, module, file, line, msg));
        // Non-blocking send — if the channel is full, drop the record
        let _ = tx.try_send(rec);
    }
}
```

Add `use std::sync::Arc;` to imports.

- [ ] **Step 2: Add log rotation support**

Add rotation check to `spawn_flusher`. When file exceeds 10 MB, rotate.

Add helper at top of `src/log/mod.rs`:

```rust
fn file_size(path: &str) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024; // 10 MB

fn rotate_log(path: &str) -> io::Result<()> {
    for i in (1..5).rev() {
        let from = format!("{}.{}", path, i);
        let to   = format!("{}.{}", path, i + 1);
        if Path::new(&from).exists() {
            fs::rename(&from, &to)?;
        }
    }
    if Path::new(path).exists() {
        fs::rename(path, format!("{}.1", path))?;
    }
    Ok(())
}
```

In `spawn_flusher`, after each flush, check file size:

```rust
if file_size(&path) > MAX_LOG_SIZE {
    let _ = rotate_log(&path);
}
```

- [ ] **Step 3: Create `src/log/macros.rs`**

```rust
//! Logging macros.
//!
//!     info!("hello {}", "world");
//!     debug!("tool result: {:?}", result);
//!     error!("failed: {}", e);

#[macro_export]
macro_rules! log {
    ($level:expr, $msg:expr $(,)?) => {
        $crate::log::log($level, $crate::log::extract_module(file!()), file!(), line!(), $msg)
    };
}

#[macro_export]
macro_rules! info {
    ($($args:tt)*) => { $crate::log::log($crate::log::Level::Info, $crate::log::extract_module(file!()), file!(), line!(), format!($($args)*)) };
}

#[macro_export]
macro_rules! debug {
    ($($args:tt)*) => { $crate::log::log($crate::log::Level::Debug, $crate::log::extract_module(file!()), file!(), line!(), format!($($args)*)) };
}

#[macro_export]
macro_rules! warn {
    ($($args:tt)*) => { $crate::log::log($crate::log::Level::Warn, $crate::log::extract_module(file!()), file!(), line!(), format!($($args)*)) };
}

#[macro_export]
macro_rules! error {
    ($($args:tt)*) => { $crate::log::log($crate::log::Level::Error, $crate::log::extract_module(file!()), file!(), line!(), format!($($args)*)) };
}

#[macro_export]
macro_rules! trace {
    ($($args:tt)*) => { $crate::log::log($crate::log::Level::Trace, $crate::log::extract_module(file!()), file!(), line!(), format!($($args)*)) };
}
```

- [ ] **Step 4: Add `mod macros;` to `src/log/mod.rs`**

Add as last line of `src/log/mod.rs`:
```rust
mod macros;
```

- [ ] **Step 5: Verify build**

Run: `cargo build 2>&1`
Expected: compiles without errors

- [ ] **Step 6: Commit**

```bash
git add src/log/mod.rs src/log/macros.rs
git commit -m "$(cat <<'EOF'
feat(log): add async flush thread, rotation, and logging macros

- Background thread batches records (256 or 1s timeout)
- Log rotation at 10MB with max 5 archived files
- info!, debug!, warn!, error!, trace! macros with file/line capture

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Integration test — verify log file output

**Files:**
- Create: `tests/log_integration_test.rs`

- [ ] **Step 1: Write integration test**

Create `tests/log_integration_test.rs`:

```rust
//! Integration test: verify log file is written correctly.

use std::fs;
use std::io::Read;
use viv::log::{init, Level};

#[test]
fn log_file_written() {
    let path = "/tmp/viv_test.log";
    let _ = fs::remove_file(path);

    init(path, Level::Debug).expect("init");
    viv::info!("integration test message {}", 42);
    viv::debug!("debug level {}", true);
    viv::error!("error level");

    // Give the flush thread time to write
    std::thread::sleep(std::time::Duration::from_millis(1500));

    let mut contents = String::new();
    fs::File::open(path).expect("open log")
        .read_to_string(&mut contents)
        .expect("read log");

    assert!(contents.contains("integration test message 42"));
    assert!(contents.contains("debug level true"));
    assert!(contents.contains("error level"));
    assert!(contents.contains("INFO"));
    assert!(contents.contains("DEBUG"));
    assert!(contents.contains("ERROR"));

    let _ = fs::remove_file(path);
}

#[test]
fn log_level_filter() {
    let path = "/tmp/viv_test_filter.log";
    let _ = fs::remove_file(path);

    init(path, Level::Warn).expect("init");
    viv::info!("should not appear");
    viv::error!("should appear");

    std::thread::sleep(std::time::Duration::from_millis(1500));

    let mut contents = String::new();
    fs::File::open(path).expect("open log")
        .read_to_string(&mut contents)
        .expect("read log");

    assert!(!contents.contains("should not appear"));
    assert!(contents.contains("should appear"));

    let _ = fs::remove_file(path);
}
```

- [ ] **Step 2: Run integration test**

Run: `cargo test --test log_integration_test 2>&1`
Expected: both tests PASS

- [ ] **Step 3: Commit**

```bash
git add tests/log_integration_test.rs
git commit -m "$(cat <<'EOF'
test(log): add integration tests for file output and level filtering

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```

---

## Self-Review Checklist

- [ ] Spec coverage: five levels ✓, async flush ✓, module extraction ✓, rotation ✓, buffered writes ✓
- [ ] No placeholders: all code is concrete, no TBD/TODO
- [ ] Type consistency: `Level` enum with `from_str`, `as_str`, `Ord`; `Record::new` with same field names throughout
- [ ] Module extraction: strips `src/` prefix, strips `.rs` suffix, converts `/` to `.`
- [ ] `mod macros` placed at end of `mod.rs` (Rust module convention)
- [ ] `log()` uses `try_send` (non-blocking) so callers never block on slow IO
