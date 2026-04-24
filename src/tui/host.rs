//! Host environment info injected into the TUI session.
//!
//! Shell name and platform string are surfaced in the welcome header.
//! They're read once at startup via [`HostInfo::from_env`] and threaded
//! through `TuiSession` so tests can override them without touching
//! process-global env vars.

/// Shell + platform strings shown in the welcome header.
#[derive(Debug, Clone)]
pub struct HostInfo {
    /// Short shell name (e.g. `"zsh"`, `"bash"`, or `"-"` when unknown).
    pub shell: String,
    /// Platform string, typically `"{os} {arch}"` (e.g. `"linux x86_64"`).
    pub platform: String,
}

impl HostInfo {
    /// Reads shell from `$SHELL` (basename only) and platform from the
    /// compile-time OS/ARCH constants. Used once at production startup.
    pub fn from_env() -> Self {
        let shell = std::env::var("SHELL")
            .ok()
            .and_then(|s| s.rsplit('/').next().map(|n| n.to_string()))
            .unwrap_or_else(|| "-".to_string());
        let platform = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
        HostInfo { shell, platform }
    }
}

impl Default for HostInfo {
    /// Fixed defaults used by tests: `"zsh"` + `"linux x86_64"`.
    /// Production code must call [`HostInfo::from_env`] instead.
    fn default() -> Self {
        HostInfo {
            shell: "zsh".to_string(),
            platform: "linux x86_64".to_string(),
        }
    }
}
