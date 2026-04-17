use std::path::PathBuf;
use crate::error::Error;
use crate::json::JsonValue;
use crate::tools::{PermissionLevel, Tool};

pub struct TodoWriteTool { path: PathBuf }
impl TodoWriteTool {
    pub fn new(path: PathBuf) -> Self { TodoWriteTool { path } }
}

impl Tool for TodoWriteTool {
    fn name(&self) -> &str { "TodoWrite" }

    fn description(&self) -> &str {
        "Use this tool to create and manage a structured task list for the current coding session. This helps track progress and organize complex tasks.\n\nEach todo requires: id (string), content (description), status (pending | in_progress | completed), and optionally priority (high | medium | low).\n\nWhen status changes, update the list via this tool. Mark tasks in_progress BEFORE beginning work; mark completed immediately after finishing."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "todos":{
                    "type":"array",
                    "description":"The complete list of todos (replaces existing list)",
                    "items":{
                        "type":"object",
                        "properties":{
                            "id":{"type":"string","description":"Unique identifier for this todo"},
                            "content":{"type":"string","description":"Description of the task"},
                            "status":{"type":"string","description":"pending | in_progress | completed"},
                            "priority":{"type":"string","description":"high | medium | low"}
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
    fn name(&self) -> &str { "TodoRead" }

    fn description(&self) -> &str {
        "Use this tool to read the current to-do list for the session. Returns a JSON array of todos. Call this at the start of each response to stay synchronized with the task list."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{"type":"object","properties":{}}"#).unwrap()
    }

    fn execute(&self, _input: &JsonValue) -> crate::Result<String> {
        if !self.path.exists() { return Ok("[]".into()); }
        std::fs::read_to_string(&self.path).map_err(|e| Error::Tool(e.to_string()))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }
}
