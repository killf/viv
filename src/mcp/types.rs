use crate::core::json::JsonValue;
use crate::Error;

// ---------------------------------------------------------------------------
// Server capabilities from initialize response
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ServerCapabilities {
    pub tools: Option<ToolsCapability>,
    pub resources: Option<ResourcesCapability>,
    pub prompts: Option<PromptsCapability>,
}

#[derive(Clone)]
pub struct ToolsCapability {
    pub list_changed: bool,
}

#[derive(Clone)]
pub struct ResourcesCapability {
    pub subscribe: bool,
    pub list_changed: bool,
}

#[derive(Clone)]
pub struct PromptsCapability {
    pub list_changed: bool,
}

impl ServerCapabilities {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        Ok(ServerCapabilities {
            tools: match json.get("tools") {
                Some(v) => Some(ToolsCapability::from_json(v)?),
                None => None,
            },
            resources: match json.get("resources") {
                Some(v) => Some(ResourcesCapability::from_json(v)?),
                None => None,
            },
            prompts: match json.get("prompts") {
                Some(v) => Some(PromptsCapability::from_json(v)?),
                None => None,
            },
        })
    }
}

impl ToolsCapability {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        Ok(ToolsCapability {
            list_changed: json
                .get("listChanged")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        })
    }
}

impl ResourcesCapability {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        Ok(ResourcesCapability {
            subscribe: json
                .get("subscribe")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            list_changed: json
                .get("listChanged")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        })
    }
}

impl PromptsCapability {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        Ok(PromptsCapability {
            list_changed: json
                .get("listChanged")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        })
    }
}

// ---------------------------------------------------------------------------
// MCP tool definition
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: JsonValue,
}

impl McpTool {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let name = json
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Json("McpTool missing 'name'".to_string()))?
            .to_string();

        let description = json
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let input_schema = json
            .get("inputSchema")
            .cloned()
            .unwrap_or(JsonValue::Object(vec![]));

        Ok(McpTool {
            name,
            description,
            input_schema,
        })
    }

    pub fn parse_list(json: &JsonValue) -> crate::Result<Vec<Self>> {
        let arr = json
            .get("tools")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::Json("missing 'tools' array".to_string()))?;

        arr.iter().map(McpTool::from_json).collect()
    }
}

// ---------------------------------------------------------------------------
// MCP resource
// ---------------------------------------------------------------------------

#[derive(Clone)]
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
            .ok_or_else(|| Error::Json("McpResource missing 'uri'".to_string()))?
            .to_string();

        let name = json
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Json("McpResource missing 'name'".to_string()))?
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

    pub fn parse_list(json: &JsonValue) -> crate::Result<Vec<Self>> {
        let arr = json
            .get("resources")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::Json("missing 'resources' array".to_string()))?;

        arr.iter().map(McpResource::from_json).collect()
    }
}

// ---------------------------------------------------------------------------
// MCP prompt
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct McpPrompt {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Vec<PromptArgument>,
}

#[derive(Clone)]
pub struct PromptArgument {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
}

impl McpPrompt {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let name = json
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Json("McpPrompt missing 'name'".to_string()))?
            .to_string();

        let description = json
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let arguments = match json.get("arguments").and_then(|v| v.as_array()) {
            Some(arr) => arr
                .iter()
                .map(PromptArgument::from_json)
                .collect::<crate::Result<Vec<_>>>()?,
            None => Vec::new(),
        };

        Ok(McpPrompt {
            name,
            description,
            arguments,
        })
    }

    pub fn parse_list(json: &JsonValue) -> crate::Result<Vec<Self>> {
        let arr = json
            .get("prompts")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::Json("missing 'prompts' array".to_string()))?;

        arr.iter().map(McpPrompt::from_json).collect()
    }
}

impl PromptArgument {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let name = json
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Json("PromptArgument missing 'name'".to_string()))?
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

// ---------------------------------------------------------------------------
// Tool call result
// ---------------------------------------------------------------------------

pub struct ToolCallResult {
    pub content: Vec<ContentItem>,
    pub is_error: bool,
}

pub enum ContentItem {
    Text(String),
    Image { data: String, mime_type: String },
    Resource { uri: String, text: String },
}

impl ToolCallResult {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let is_error = json
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let content_arr = json
            .get("content")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::Json("ToolCallResult missing 'content'".to_string()))?;

        let content = content_arr
            .iter()
            .map(ContentItem::from_json)
            .collect::<crate::Result<Vec<_>>>()?;

        Ok(ToolCallResult { content, is_error })
    }

    pub fn to_text(&self) -> String {
        let parts: Vec<&str> = self
            .content
            .iter()
            .filter_map(|item| match item {
                ContentItem::Text(t) => Some(t.as_str()),
                ContentItem::Resource { text, .. } => Some(text.as_str()),
                ContentItem::Image { .. } => None,
            })
            .collect();

        parts.join("\n")
    }
}

impl ContentItem {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let content_type = json
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Json("ContentItem missing 'type'".to_string()))?;

        match content_type {
            "text" => {
                let text = json
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::Json("text ContentItem missing 'text'".to_string()))?
                    .to_string();
                Ok(ContentItem::Text(text))
            }
            "image" => {
                let data = json
                    .get("data")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::Json("image ContentItem missing 'data'".to_string()))?
                    .to_string();
                let mime_type = json
                    .get("mimeType")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        Error::Json("image ContentItem missing 'mimeType'".to_string())
                    })?
                    .to_string();
                Ok(ContentItem::Image { data, mime_type })
            }
            "resource" => {
                let resource = json
                    .get("resource")
                    .ok_or_else(|| {
                        Error::Json("resource ContentItem missing 'resource'".to_string())
                    })?;
                let uri = resource
                    .get("uri")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::Json("resource missing 'uri'".to_string()))?
                    .to_string();
                let text = resource
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::Json("resource missing 'text'".to_string()))?
                    .to_string();
                Ok(ContentItem::Resource { uri, text })
            }
            other => Err(Error::Json(format!(
                "unknown ContentItem type: '{}'",
                other
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Resource content
// ---------------------------------------------------------------------------

pub struct ResourceContent {
    pub contents: Vec<ResourceContentItem>,
}

pub enum ResourceContentItem {
    Text {
        uri: String,
        mime_type: Option<String>,
        text: String,
    },
    Blob {
        uri: String,
        mime_type: Option<String>,
        blob: String,
    },
}

impl ResourceContent {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let contents_arr = json
            .get("contents")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::Json("ResourceContent missing 'contents'".to_string()))?;

        let contents = contents_arr
            .iter()
            .map(ResourceContentItem::from_json)
            .collect::<crate::Result<Vec<_>>>()?;

        Ok(ResourceContent { contents })
    }
}

impl ResourceContentItem {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let uri = json
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Json("ResourceContentItem missing 'uri'".to_string()))?
            .to_string();

        let mime_type = json
            .get("mimeType")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Distinguish text vs blob by which field is present
        if let Some(text) = json.get("text").and_then(|v| v.as_str()) {
            Ok(ResourceContentItem::Text {
                uri,
                mime_type,
                text: text.to_string(),
            })
        } else if let Some(blob) = json.get("blob").and_then(|v| v.as_str()) {
            Ok(ResourceContentItem::Blob {
                uri,
                mime_type,
                blob: blob.to_string(),
            })
        } else {
            Err(Error::Json(
                "ResourceContentItem has neither 'text' nor 'blob'".to_string(),
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// Prompt messages
// ---------------------------------------------------------------------------

pub struct PromptMessages {
    pub description: Option<String>,
    pub messages: Vec<PromptMessage>,
}

pub struct PromptMessage {
    pub role: String,
    pub content: ContentItem,
}

impl PromptMessages {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let description = json
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let messages_arr = json
            .get("messages")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::Json("PromptMessages missing 'messages'".to_string()))?;

        let messages = messages_arr
            .iter()
            .map(PromptMessage::from_json)
            .collect::<crate::Result<Vec<_>>>()?;

        Ok(PromptMessages {
            description,
            messages,
        })
    }
}

impl PromptMessage {
    pub fn from_json(json: &JsonValue) -> crate::Result<Self> {
        let role = json
            .get("role")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Json("PromptMessage missing 'role'".to_string()))?
            .to_string();

        let content_json = json
            .get("content")
            .ok_or_else(|| Error::Json("PromptMessage missing 'content'".to_string()))?;

        let content = ContentItem::from_json(content_json)?;

        Ok(PromptMessage { role, content })
    }
}
