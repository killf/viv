# Zero-Dependency Logging System Design

**Date:** 2026-04-18
**Status:** Approved

## Overview

A self-contained, zero-dependency logging system for `viv`, providing structured log output to files with async write performance via a background thread. Supports five log levels, automatic module labeling, and log rotation.

## Architecture

```
log::init(path, level)           # Initialize global logger
        │
        ▼
┌───────────────────────────────────────────────────────┐
│  Global Logger (static Lazy<Mutex<Logger>>)           │
│                                                       │
│  tx: mpsc::Sender<Arc<Record>>                       │
│  └── spawns background flush thread                  │
│                                                       │
│  Buffer: Vec<Arc<Record>>                            │
│  └── flushes on: 256 records OR 1s timeout OR drop   │
│                                                       │
│  File: std::fs::OpenOptions + std::io::BufWriter     │
│  └── append mode, buffered writes                     │
└───────────────────────────────────────────────────────┘

Call sites:
  info!("message")      → macro captures file!(), line!, level
  debug!(...)          → compile-time filter if level < LOG_LEVEL
  error!(...)
```

## Components

### Level

Five severity levels matching standard practice:

```rust
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error = 0,
    Warn  = 1,
    Info  = 2,
    Debug = 3,
    Trace = 4,
}
```

### Record

Single log entry with all metadata:

```rust
pub struct Record {
    pub time:   DateTime<Utc>,  // RFC 3339 timestamp
    pub level:  Level,
    pub module: &'static str,   // extracted from file!() at compile time
    pub file:   &'static str,  // __FILE__
    pub line:   u32,            // __LINE__
    pub msg:    String,
}
```

Module extraction: take `file!()`, strip `src/` prefix and `/mod.rs` or `.rs` suffix.
Example: `src/agent/mod.rs` → `agent`, `src/llm.rs` → `llm`.

### Logger

Manages the background flush thread and file output:

```rust
pub struct Logger {
    file:   RefCell<std::fs::File>,
    writer: RefCell<std::io::BufWriter<std::fs::File>>,
    buf:    RefCell<Vec<u8>>,         // batch buffer
    level:  Level,
}

static LOGGER: Lazy<Mutex<Option<Logger>>> = Lazy::new(|| Mutex::new(None));
```

Flush triggers:
- Buffer reaches 256 records
- 1 second elapsed since last flush
- `Logger` dropped (flush remaining)

### Macros

```rust
#[macro_export]
macro_rules! log {
    ($level:expr, $msg:expr $(,)?) => {
        $crate::log::log($level, module_name!(), file!(), line!(), $msg)
    };
}

#[macro_export]
macro_rules! info  { ($($args:tt)*) => { log!(Level::Info,  format!($($args)*)) } }
#[macro_export]
macro_rules! debug { ($($args:tt)*) => { log!(Level::Debug, format!($($args)*)) } }
#[macro_export]
macro_rules! warn  { ($($args:tt)*) => { log!(Level::Warn,  format!($($args)*)) } }
#[macro_export]
macro_rules! error { ($($args:tt)*) => { log!(Level::Error, format!($($args)*)) } }
#[macro_export]
macro_rules! trace { ($($args:tt)*) => { log!(Level::Trace, format!($($args)*)) } }
```

Compile-time filtering: `info!`/`debug!` etc. still call `log!`; `log!` checks runtime level.
Note: `module_name!` is a custom macro that extracts module from `file!()`.

### Log Format

```
2026-04-18T10:30:45.123Z ERROR [agent] connection timeout (src/agent/mod.rs:42)
2026-04-18T10:30:45.456Z DEBUG [llm] prompt tokens: 1234
2026-04-18T10:30:46.000Z INFO  [lsp] file changed: foo.rs
```

Format: `{time} {LEVEL:<5} [{module}] {msg} ({file}:{line})\n`

## Log Rotation

- **Size-based**: rotate when current file exceeds 10 MB
- **Pattern**: `viv.log` → `viv.log.1` → `viv.log.2` (max 5 files)
- Rotates on `init()` and when size limit reached

## File Location

- Default: `viv.log` in current working directory
- Configurable via `init(path, level)`

## Error Handling

- `init` returns `Result<()>` if file cannot be opened/written
- Log errors (e.g., write failure) are silently ignored to prevent log failure cascading
- Background thread logs to stderr on flush errors before terminating

## Module Structure

```
src/
└── log/
    ├── mod.rs         # Logger, Level, Record, init()
    └── macros.rs      # log!, info!, debug!, etc.
```

Exposed via `src/lib.rs` as `pub mod log`.

## Testing

- Unit tests for module extraction
- Unit tests for level filtering
- Unit tests for Record formatting

## Implementation Notes

- Uses `std::sync::OnceLock` for global singleton (stdlib, no external deps)
- Timestamp: `SystemTime::now()` + manual RFC 3339 formatting (no chrono)
- No `chrono`, no `time` crate — format timestamps manually

## Open Questions

- [x] Module label extraction method — resolved: from `file!()` macro
- [x] Flush policy — resolved: 256 records or 1 second timeout
- [x] Rotation strategy — resolved: 10MB size limit, max 5 files
