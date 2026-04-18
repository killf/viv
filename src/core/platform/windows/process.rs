use std::process::Command;

pub fn shell_command(cmd: &str) -> Command {
    let mut c = Command::new("powershell");
    c.args(["-NoProfile", "-NonInteractive", "-Command", cmd]);
    c
}
