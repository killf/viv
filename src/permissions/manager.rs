use std::collections::HashSet;
use crate::core::json::JsonValue;
use crate::tools::{Tool, PermissionLevel};

#[derive(Default)]
pub struct PermissionManager {
    session_allowed: HashSet<String>,
}

impl PermissionManager {
    pub fn check(
        &mut self,
        tool: &dyn Tool,
        input: &JsonValue,
        ask_fn: &mut dyn FnMut(&str, &JsonValue) -> bool,
    ) -> bool {
        if tool.permission_level() == PermissionLevel::ReadOnly {
            return true;
        }
        if self.session_allowed.contains(tool.name()) {
            return true;
        }
        let granted = ask_fn(tool.name(), input);
        if granted {
            self.session_allowed.insert(tool.name().to_string());
        }
        granted
    }
}
