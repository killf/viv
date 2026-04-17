use crate::Error;
use crate::core::json::JsonValue;

/// Server capabilities returned from initialize
#[derive(Debug, Clone)]
pub struct ServerCapabilities {
    pub tools: bool,
    pub resources: bool,
    pub prompts: bool,
}

impl ServerCapabilities {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let caps = json.get("capabilities").unwrap_or(json);
        Ok(ServerCapabilities {
            tools: caps.get("tools").is_some(),
            resources: caps.get("resources").is_some(),
            prompts: caps.get("prompts").is_some(),
        })
    }
}

/// Input schema for a tool parameter
#[derive(Debug, Clone)]
pub struct ToolInputSchema {
    pub schema_type: String,
    pub properties: Vec<(String, JsonValue)>,
    pub required: Vec<String>,
}

impl ToolInputSchema {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let schema_type = json
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("object")
            .to_string();
        let properties = if let Some(props) = json.get("properties") {
            if let Some(pairs) = props.as_object() {
                pairs.clone()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        let required = if let Some(req) = json.get("required") {
            if let Some(arr) = req.as_array() {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        Ok(ToolInputSchema {
            schema_type,
            properties,
            required,
        })
    }
}

/// MCP tool definition
#[derive(Debug, Clone)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: ToolInputSchema,
}

impl McpTool {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let name = json
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Json("McpTool missing name".to_string()))?
            .to_string();
        let description = json
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let input_schema = if let Some(schema) = json.get("inputSchema") {
            ToolInputSchema::from_json(schema)?
        } else {
            ToolInputSchema {
                schema_type: "object".to_string(),
                properties: Vec::new(),
                required: Vec::new(),
            }
        };
        Ok(McpTool {
            name,
            description,
            input_schema,
        })
    }

    pub fn parse_list(json: &JsonValue) -> crate::Result<Vec<McpTool>> {
        let arr = json
            .as_array()
            .ok_or_else(|| Error::Json("Expected array of tools".to_string()))?;
        arr.iter().map(McpTool::from_json).collect()
    }
}

/// Content item in a tool call result
#[derive(Debug, Clone)]
pub struct ContentItem {
    pub content_type: String,
    pub text: String,
}

impl ContentItem {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let content_type = json
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("text")
            .to_string();
        let text = json
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok(ContentItem { content_type, text })
    }
}

/// Result of calling a tool
#[derive(Debug, Clone)]
pub struct ToolCallResult {
    pub content: Vec<ContentItem>,
    pub is_error: bool,
}

impl ToolCallResult {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let is_error = json
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let content = if let Some(arr) = json.get("content").and_then(|v| v.as_array()) {
            arr.iter()
                .map(ContentItem::from_json)
                .collect::<crate::Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        Ok(ToolCallResult { content, is_error })
    }
}

/// MCP resource definition
#[derive(Debug, Clone)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

impl McpResource {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let uri = json
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Json("McpResource missing uri".to_string()))?
            .to_string();
        let name = json
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Json("McpResource missing name".to_string()))?
            .to_string();
        let description = json
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let mime_type = json
            .get("mimeType")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Ok(McpResource {
            uri,
            name,
            description,
            mime_type,
        })
    }

    pub fn parse_list(json: &JsonValue) -> crate::Result<Vec<McpResource>> {
        let arr = json
            .as_array()
            .ok_or_else(|| Error::Json("Expected array of resources".to_string()))?;
        arr.iter().map(McpResource::from_json).collect()
    }
}

/// Content of a read resource
#[derive(Debug, Clone)]
pub struct ResourceContent {
    pub uri: String,
    pub mime_type: Option<String>,
    pub text: Option<String>,
    pub blob: Option<String>,
}

impl ResourceContent {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let uri = json
            .get("uri")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mime_type = json
            .get("mimeType")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let text = json
            .get("text")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let blob = json
            .get("blob")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Ok(ResourceContent {
            uri,
            mime_type,
            text,
            blob,
        })
    }
}

/// MCP prompt definition
#[derive(Debug, Clone)]
pub struct McpPrompt {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Vec<PromptArgument>,
}

impl McpPrompt {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let name = json
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Json("McpPrompt missing name".to_string()))?
            .to_string();
        let description = json
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let arguments = if let Some(arr) = json.get("arguments").and_then(|v| v.as_array()) {
            arr.iter()
                .map(PromptArgument::from_json)
                .collect::<crate::Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        Ok(McpPrompt {
            name,
            description,
            arguments,
        })
    }

    pub fn parse_list(json: &JsonValue) -> crate::Result<Vec<McpPrompt>> {
        let arr = json
            .as_array()
            .ok_or_else(|| Error::Json("Expected array of prompts".to_string()))?;
        arr.iter().map(McpPrompt::from_json).collect()
    }
}

/// Prompt argument definition
#[derive(Debug, Clone)]
pub struct PromptArgument {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
}

impl PromptArgument {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let name = json
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Json("PromptArgument missing name".to_string()))?
            .to_string();
        let description = json
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let required = json
            .get("required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        Ok(PromptArgument {
            name,
            description,
            required,
        })
    }
}

/// Prompt message (result of get_prompt)
#[derive(Debug, Clone)]
pub struct PromptMessage {
    pub role: String,
    pub content: String,
}

impl PromptMessage {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let role = json
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("user")
            .to_string();
        let content = if let Some(c) = json.get("content") {
            if let Some(s) = c.as_str() {
                s.to_string()
            } else if let Some(obj) = c.as_object() {
                // content might be {type: "text", text: "..."}
                let _ = obj;
                c.get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        Ok(PromptMessage { role, content })
    }
}

/// Collection of prompt messages returned from get_prompt
#[derive(Debug, Clone)]
pub struct PromptMessages {
    pub description: Option<String>,
    pub messages: Vec<PromptMessage>,
}

impl PromptMessages {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let description = json
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let messages = if let Some(arr) = json.get("messages").and_then(|v| v.as_array()) {
            arr.iter()
                .map(PromptMessage::from_json)
                .collect::<crate::Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        Ok(PromptMessages {
            description,
            messages,
        })
    }
}
