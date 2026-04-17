use std::path::PathBuf;
use crate::error::Error;
use crate::json::JsonValue;
use crate::tools::{PermissionLevel, Tool};

pub struct TodoWriteTool { path: PathBuf }
impl TodoWriteTool {
    pub fn new(path: PathBuf) -> Self { TodoWriteTool { path } }
}

impl Tool for TodoWriteTool {
    fn name(&self) -> &str { "todo_write" }

    fn description(&self) -> &str {
        "Write the task list. Replaces all existing todos. Each todo needs id, content, and status (pending | in_progress | completed)."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "todos":{
                    "type":"array",
                    "items":{
                        "type":"object",
                        "properties":{
                            "id":{"type":"string"},
                            "content":{"type":"string"},
                            "status":{"type":"string"}
                        },
                        "required":["id","content","status"]
                    }
                }
            },
            "required":["todos"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let todos = input.get("todos").ok_or_else(|| Error::Tool("missing 'todos'".into()))?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::Tool(e.to_string()))?;
        }
        std::fs::write(&self.path, format!("{}", todos))
            .map_err(|e| Error::Tool(e.to_string()))?;
        let count = todos.as_array().map(|a| a.len()).unwrap_or(0);
        Ok(format!("Wrote {} todo(s)", count))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Write }
}

pub struct TodoReadTool { path: PathBuf }
impl TodoReadTool {
    pub fn new(path: PathBuf) -> Self { TodoReadTool { path } }
}

impl Tool for TodoReadTool {
    fn name(&self) -> &str { "todo_read" }
    fn description(&self) -> &str { "Read the current task list. Returns a JSON array of todos." }
    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{"type":"object","properties":{}}"#).unwrap()
    }

    fn execute(&self, _input: &JsonValue) -> crate::Result<String> {
        if !self.path.exists() { return Ok("[]".into()); }
        std::fs::read_to_string(&self.path).map_err(|e| Error::Tool(e.to_string()))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }
}
