use std::collections::HashMap;
use std::os::unix::io::AsRawFd;
use std::process::{Command, Stdio};

use crate::core::platform::types::RawHandle;

pub fn shell_command(cmd: &str) -> Command {
    let mut c = Command::new("sh");
    c.arg("-c").arg(cmd);
    c
}

pub struct ChildProcess {
    pub child: std::process::Child,
    pub stdin_fd: RawHandle,
    pub stdout_fd: RawHandle,
}

pub fn spawn_piped(cmd: &str, args: &[&str]) -> crate::Result<ChildProcess> {
    spawn_piped_with_env(cmd, args, &HashMap::new())
}

pub fn spawn_piped_with_env(
    cmd: &str,
    args: &[&str],
    env: &HashMap<String, String>,
) -> crate::Result<ChildProcess> {
    let mut child = Command::new(cmd)
        .args(args)
        .envs(env.iter())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(crate::Error::Io)?;
    let stdin_fd = child
        .stdin
        .as_ref()
        .ok_or_else(|| crate::Error::Invariant("spawn_piped: no stdin".into()))?
        .as_raw_fd();
    let stdout_fd = child
        .stdout
        .as_ref()
        .ok_or_else(|| crate::Error::Invariant("spawn_piped: no stdout".into()))?
        .as_raw_fd();
    Ok(ChildProcess {
        child,
        stdin_fd,
        stdout_fd,
    })
}
