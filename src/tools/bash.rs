use crate::core::json::JsonValue;
use crate::core::platform::shell_command;
use crate::error::Error;
use crate::tools::{PermissionLevel, Tool};
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::time::{Duration, Instant};

pub struct BashTool;

impl Tool for BashTool {
    fn name(&self) -> &str {
        "Bash"
    }

    fn description(&self) -> &str {
        "Executes a given bash command and returns its output.\n\nIMPORTANT: Avoid using this tool to run `find`, `grep`, `cat`, `head`, `tail`, `sed`, `awk`, or `echo` commands — use the dedicated tools instead.\n\n- Quote file paths containing spaces with double quotes\n- Try to maintain current working directory using absolute paths\n- You may specify an optional timeout in milliseconds (up to 600000ms / 10 minutes)\n- Use run_in_background for fire-and-forget processes; no need for `&` at the end"
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "command":{"type":"string","description":"The command to execute"},
                "timeout":{"type":"number","description":"Optional timeout in milliseconds (max 600000). Default: 120000."},
                "description":{"type":"string","description":"Clear, concise description of what this command does in active voice."},
                "run_in_background":{"type":"boolean","description":"Set to true to run the command in the background. Returns the PID. Default: false."},
                "dangerouslyDisableSandbox":{"type":"boolean","description":"Set to true to run without sandboxing. Default: false."}
            },
            "required":["command"]
        }"#).unwrap()
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let input = input.clone();
        Box::pin(async move {
            let command = input
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool("missing 'command'".into()))?;
            let timeout_ms = input
                .get("timeout")
                .and_then(|v| v.as_i64())
                .unwrap_or(120_000)
                .min(600_000) as u64;
            let run_in_background = input
                .get("run_in_background")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if run_in_background {
                let child = shell_command(command)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|e| Error::Tool(format!("spawn: {}", e)))?;
                return Ok(format!("Background process started (pid: {})", child.id()));
            }

            let mut child = shell_command(command)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| Error::Tool(format!("spawn: {}", e)))?;

            let deadline = Instant::now() + Duration::from_millis(timeout_ms);
            loop {
                if child
                    .try_wait()
                    .map_err(|e| Error::Tool(e.to_string()))?
                    .is_some()
                {
                    break;
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    return Err(Error::Tool(format!("timed out after {}ms", timeout_ms)));
                }
                std::thread::sleep(Duration::from_millis(50));
            }

            let output = child
                .wait_with_output()
                .map_err(|e| Error::Tool(e.to_string()))?;
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let code = output.status.code().unwrap_or(-1);

            let mut result = stdout;
            if !stderr.is_empty() {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(&stderr);
            }
            if code != 0 {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(&format!("Exit code: {}", code));
            }
            Ok(result)
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Execute
    }
}
