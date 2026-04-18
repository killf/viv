use crate::core::json::JsonValue;
use crate::error::Error;
use crate::tools::{PermissionLevel, Tool};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

pub struct TodoWriteTool {
    path: PathBuf,
}
impl TodoWriteTool {
    pub fn new(path: PathBuf) -> Self {
        TodoWriteTool { path }
    }
}

impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "TodoWrite"
    }

    fn description(&self) -> &str {
        "Use this tool to create and manage a structured task list for the current coding session.\n\nTask descriptions must have two forms:\n- content: The imperative form describing what needs to be done (e.g., \"Run tests\")\n- activeForm: The present continuous form shown during execution (e.g., \"Running tests\")\n\nTask states: pending, in_progress, completed. Only one task should be in_progress at a time. Mark tasks complete immediately after finishing."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "todos":{
                    "type":"array",
                    "description":"The updated todo list",
                    "items":{
                        "type":"object",
                        "properties":{
                            "content":{"type":"string","description":"Description of the task"},
                            "status":{"type":"string","description":"pending | in_progress | completed"},
                            "activeForm":{"type":"string","description":"Present continuous form shown during execution (e.g. 'Running tests')"}
                        },
                        "required":["content","status","activeForm"]
                    }
                }
            },
            "required":["todos"]
        }"#).unwrap()
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let input = input.clone();
        let path = self.path.clone();
        Box::pin(async move {
            let todos = input
                .get("todos")
                .ok_or_else(|| Error::Tool("missing 'todos'".into()))?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| Error::Tool(e.to_string()))?;
            }
            std::fs::write(&path, format!("{}", todos)).map_err(|e| Error::Tool(e.to_string()))?;
            let count = todos.as_array().map(|a| a.len()).unwrap_or(0);
            Ok(format!("Wrote {} todo(s)", count))
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Write
    }
}

pub struct TodoReadTool {
    path: PathBuf,
}
impl TodoReadTool {
    pub fn new(path: PathBuf) -> Self {
        TodoReadTool { path }
    }
}

impl Tool for TodoReadTool {
    fn name(&self) -> &str {
        "TodoRead"
    }

    fn description(&self) -> &str {
        "Use this tool to read the current to-do list for the session. Returns a JSON array of todos. Call this at the start of each response to stay synchronized with the task list."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{"type":"object","properties":{}}"#).unwrap()
    }

    fn execute(
        &self,
        _input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let path = self.path.clone();
        Box::pin(async move {
            if !path.exists() {
                return Ok("[]".into());
            }
            std::fs::read_to_string(&path).map_err(|e| Error::Tool(e.to_string()))
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}
