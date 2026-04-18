//! Logging macros.
//!
//! ```ignore
//! use viv::log::Level;
//! viv::log::init("viv.log", Level::Info)?;
//! info!("hello {}", "world");
//! debug!("tool result: {:?}", result);
//! error!("failed: {}", e);
//! ```

/// Core log macro. Dispatches to the global logger via the channel.
#[macro_export]
macro_rules! log {
    ($level:expr, $msg:expr $(,)?) => {
        $crate::log::log(
            $level,
            $crate::log::extract_module(file!()),
            file!(),
            line!(),
            $msg,
        )
    };
}

/// Log at INFO level.
#[macro_export]
macro_rules! info {
    ($($args:tt)*) => {
        $crate::log::log($crate::log::Level::Info, $crate::log::extract_module(file!()), file!(), line!(), format!($($args)*))
    };
}

/// Log at DEBUG level.
#[macro_export]
macro_rules! debug {
    ($($args:tt)*) => {
        $crate::log::log($crate::log::Level::Debug, $crate::log::extract_module(file!()), file!(), line!(), format!($($args)*))
    };
}

/// Log at WARN level.
#[macro_export]
macro_rules! warn {
    ($($args:tt)*) => {
        $crate::log::log($crate::log::Level::Warn, $crate::log::extract_module(file!()), file!(), line!(), format!($($args)*))
    };
}

/// Log at ERROR level.
#[macro_export]
macro_rules! error {
    ($($args:tt)*) => {
        $crate::log::log($crate::log::Level::Error, $crate::log::extract_module(file!()), file!(), line!(), format!($($args)*))
    };
}

/// Log at TRACE level.
#[macro_export]
macro_rules! trace {
    ($($args:tt)*) => {
        $crate::log::log($crate::log::Level::Trace, $crate::log::extract_module(file!()), file!(), line!(), format!($($args)*))
    };
}
