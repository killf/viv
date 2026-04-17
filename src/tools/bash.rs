use crate::error::Error;
use crate::json::JsonValue;
use crate::tools::{PermissionLevel, Tool};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct BashTool;

impl Tool for BashTool {
    fn name(&self) -> &str { "bash" }

    fn description(&self) -> &str {
        "Execute a shell command and return stdout + stderr. Fails with an error if the command times out."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "command":{"type":"string","description":"Shell command to run"},
                "timeout_ms":{"type":"number","description":"Timeout in ms (default: 30000)"}
            },
            "required":["command"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let command = input.get("command").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'command'".into()))?;
        let timeout_ms = input.get("timeout_ms").and_then(|v| v.as_i64()).unwrap_or(30_000) as u64;

        let mut child = Command::new("sh")
            .arg("-c").arg(command)
            .stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::Tool(format!("spawn: {}", e)))?;

        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            if child.try_wait().map_err(|e| Error::Tool(e.to_string()))?.is_some() { break; }
            if Instant::now() >= deadline {
                let _ = child.kill();
                return Err(Error::Tool(format!("timed out after {}ms", timeout_ms)));
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        let output = child.wait_with_output().map_err(|e| Error::Tool(e.to_string()))?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let code = output.status.code().unwrap_or(-1);

        let mut result = stdout;
        if !stderr.is_empty() {
            if !result.is_empty() { result.push('\n'); }
            result.push_str(&stderr);
        }
        if code != 0 {
            if !result.is_empty() { result.push('\n'); }
            result.push_str(&format!("Exit code: {}", code));
        }
        Ok(result)
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Execute }
}

pub struct BashBackgroundTool;

impl Tool for BashBackgroundTool {
    fn name(&self) -> &str { "bash_background" }

    fn description(&self) -> &str {
        "Start a shell command in the background and return its process ID. The process runs independently of the agent."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "command":{"type":"string","description":"Shell command to run in background"},
                "description":{"type":"string","description":"What this process does"}
            },
            "required":["command","description"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let command = input.get("command").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'command'".into()))?;
        let description = input.get("description").and_then(|v| v.as_str()).unwrap_or("");

        let child = Command::new("sh")
            .arg("-c").arg(command)
            .stdout(Stdio::null()).stderr(Stdio::null())
            .spawn()
            .map_err(|e| Error::Tool(format!("spawn: {}", e)))?;

        Ok(format!("Started background process (pid: {}): {}", child.id(), description))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Execute }
}
