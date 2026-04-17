use std::collections::HashSet;

#[derive(Default)]
pub struct PermissionManager {
    session_allowed: HashSet<String>,
}


impl PermissionManager {
    /// Returns true if the tool has already been granted in this session.
    pub fn is_allowed(&self, tool_name: &str) -> bool {
        self.session_allowed.contains(tool_name)
    }

    /// Marks the tool as allowed for the rest of this session.
    pub fn grant(&mut self, tool_name: &str) {
        self.session_allowed.insert(tool_name.to_string());
    }
}
