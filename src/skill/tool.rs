use crate::core::json::JsonValue;
use crate::error::Error;
use crate::skill::SkillRegistry;
use crate::tools::{PermissionLevel, Tool};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct SkillTool {
    registry: Arc<SkillRegistry>,
}

impl SkillTool {
    pub fn new(registry: Arc<SkillRegistry>) -> Self {
        SkillTool { registry }
    }
}

impl Tool for SkillTool {
    fn name(&self) -> &str {
        "Skill"
    }

    fn description(&self) -> &str {
        "Invoke a skill by name. Returns the skill's full content for you to follow."
    }

    fn input_schema(&self) -> JsonValue {
        crate::tools::parse_schema(
            r#"{
            "type": "object",
            "properties": {
                "skill": {
                    "type": "string",
                    "description": "The name of the skill to invoke"
                },
                "args": {
                    "type": "string",
                    "description": "Optional arguments to pass to the skill"
                }
            },
            "required": ["skill"]
        }"#,
        )
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let input = input.clone();
        let registry = Arc::clone(&self.registry);
        Box::pin(async move {
            let name = input
                .get("skill")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool("missing 'skill' field".into()))?;

            match registry.get(name) {
                Some(entry) => Ok(format!(
                    "Base directory for this skill: {}\n\n{}",
                    entry.base_dir, entry.content
                )),
                None => {
                    let available = registry.names().join(", ");
                    Ok(format!(
                        "Skill '{}' not found. Available: {}",
                        name, available
                    ))
                }
            }
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}
