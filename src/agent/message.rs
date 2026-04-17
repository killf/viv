use crate::json::JsonValue;

// ── ContentBlock ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ContentBlock {
    Text(String),
    ToolUse { id: String, name: String, input: JsonValue },
    ToolResult { tool_use_id: String, content: Vec<ContentBlock>, is_error: bool },
}

impl ContentBlock {
    pub fn to_json(&self) -> String {
        match self {
            ContentBlock::Text(t) => {
                format!("{{\"type\":\"text\",\"text\":{}}}", JsonValue::Str(t.clone()))
            }
            ContentBlock::ToolUse { id, name, input } => {
                format!(
                    "{{\"type\":\"tool_use\",\"id\":{},\"name\":{},\"input\":{}}}",
                    JsonValue::Str(id.clone()),
                    JsonValue::Str(name.clone()),
                    input,
                )
            }
            ContentBlock::ToolResult { tool_use_id, content, is_error } => {
                let content_json: Vec<String> = content.iter().map(|b| b.to_json()).collect();
                format!(
                    "{{\"type\":\"tool_result\",\"tool_use_id\":{},\"content\":[{}],\"is_error\":{}}}",
                    JsonValue::Str(tool_use_id.clone()),
                    content_json.join(","),
                    is_error,
                )
            }
        }
    }
}

// ── Message ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    User(Vec<ContentBlock>),
    Assistant(Vec<ContentBlock>),
}

impl Message {
    pub fn user_text(text: impl Into<String>) -> Self {
        Message::User(vec![ContentBlock::Text(text.into())])
    }

    pub fn role(&self) -> &str {
        match self { Message::User(_) => "user", Message::Assistant(_) => "assistant" }
    }

    pub fn blocks(&self) -> &[ContentBlock] {
        match self { Message::User(b) | Message::Assistant(b) => b }
    }

    pub fn to_json(&self) -> String {
        let blocks_json: Vec<String> = self.blocks().iter().map(|b| b.to_json()).collect();
        format!(
            "{{\"role\":{},\"content\":[{}]}}",
            JsonValue::Str(self.role().into()),
            blocks_json.join(","),
        )
    }
}

// ── SystemBlock（带 cache_control）────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SystemBlock {
    pub text: String,
    pub cached: bool,
}

impl SystemBlock {
    pub fn cached(text: impl Into<String>) -> Self {
        SystemBlock { text: text.into(), cached: true }
    }
    pub fn dynamic(text: impl Into<String>) -> Self {
        SystemBlock { text: text.into(), cached: false }
    }
    pub fn to_json(&self) -> String {
        if self.cached {
            format!(
                "{{\"type\":\"text\",\"text\":{},\"cache_control\":{{\"type\":\"ephemeral\"}}}}",
                JsonValue::Str(self.text.clone()),
            )
        } else {
            format!("{{\"type\":\"text\",\"text\":{}}}", JsonValue::Str(self.text.clone()))
        }
    }
}

// ── PromptCache（内容 hash，避免重复序列化）────────────────────────────────────

#[derive(Default)]
pub struct PromptCache {
    pub base_hash: u64,
    pub base_text: String,
    pub tools_hash: u64,
    pub tools_text: String,
    pub skills_hash: u64,
    pub skills_text: String,
}

pub fn hash_str(s: &str) -> u64 {
    // FNV-1a 64-bit
    let mut h: u64 = 14695981039346656037;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}
