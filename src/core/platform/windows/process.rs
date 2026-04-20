use std::collections::HashMap;
use std::process::Command;

use crate::core::platform::types::RawHandle;

pub fn shell_command(cmd: &str) -> Command {
    let mut c = Command::new("powershell");
    c.args(["-NoProfile", "-NonInteractive", "-Command", cmd]);
    c
}

pub struct ChildProcess {
    pub child: std::process::Child,
    pub stdin_fd: RawHandle,
    pub stdout_fd: RawHandle,
}

pub fn spawn_piped(_cmd: &str, _args: &[&str]) -> crate::Result<ChildProcess> {
    Err(crate::Error::Invariant(
        "spawn_piped not yet implemented on Windows".into(),
    ))
}

pub fn spawn_piped_with_env(
    _cmd: &str,
    _args: &[&str],
    _env: &HashMap<String, String>,
) -> crate::Result<ChildProcess> {
    Err(crate::Error::Invariant(
        "spawn_piped not yet implemented on Windows".into(),
    ))
}
