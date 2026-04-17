use std::io::{Read, Write};

use crate::json::JsonValue;
use crate::net::http::HttpRequest;
use crate::net::sse::SseParser;
use crate::net::tls::TlsStream;
use crate::error::Error;

/// A single chat message with a role and content.
#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// Model tier for selecting the appropriate Claude model.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelTier {
    /// Fast, lightweight model (e.g., Haiku) for simple tasks.
    Fast,
    /// Balanced model (e.g., Sonnet) for daily tasks.
    Medium,
    /// Most capable model (e.g., Opus) for complex reasoning.
    Slow,
}

/// Configuration for the Anthropic Claude API.
#[derive(Debug, Clone)]
pub struct LLMConfig {
    pub api_key: String,
    pub base_url: String,
    pub model_fast: String,
    pub model_medium: String,
    pub model_slow: String,
}

/// Parsed URL components from base_url.
struct UrlParts {
    host: String,
    port: u16,
    path_prefix: String,
}

fn parse_base_url(base_url: &str) -> UrlParts {
    let mut url = base_url;

    // Strip scheme
    let is_https;
    if let Some(rest) = url.strip_prefix("https://") {
        url = rest;
        is_https = true;
    } else if let Some(rest) = url.strip_prefix("http://") {
        url = rest;
        is_https = false;
    } else {
        is_https = true;
    }

    // Split host from path
    let (host_port, path_prefix) = match url.find('/') {
        Some(i) => (&url[..i], url[i..].to_string()),
        None => (url, String::new()),
    };

    // Strip trailing slash from path_prefix
    let path_prefix = path_prefix.trim_end_matches('/').to_string();

    // Split host:port
    let (host, port) = match host_port.rfind(':') {
        Some(i) => {
            let port_str = &host_port[i + 1..];
            match port_str.parse::<u16>() {
                Ok(p) => (host_port[..i].to_string(), p),
                Err(_) => (host_port.to_string(), if is_https { 443 } else { 80 }),
            }
        }
        None => (host_port.to_string(), if is_https { 443 } else { 80 }),
    };

    UrlParts { host, port, path_prefix }
}

impl LLMConfig {
    /// Build an `LLMConfig` from environment variables.
    ///
    /// - `VIV_API_KEY` — required
    /// - `VIV_BASE_URL` — optional (default: "api.anthropic.com")
    /// - `VIV_MODEL_FAST` — optional, falls back to `VIV_MODEL`, then default
    /// - `VIV_MODEL_MEDIUM` — optional, falls back to `VIV_MODEL`, then default
    /// - `VIV_MODEL_SLOW` — optional, falls back to `VIV_MODEL`, then default
    /// - `VIV_MODEL` — optional fallback for all three tiers
    pub fn from_env() -> crate::Result<Self> {
        let api_key = std::env::var("VIV_API_KEY").map_err(|_| Error::LLM {
            status: 0,
            message: "VIV_API_KEY not set".into(),
        })?;

        let base_url = std::env::var("VIV_BASE_URL")
            .unwrap_or_else(|_| "api.anthropic.com".into());

        let fallback_model = std::env::var("VIV_MODEL").ok();

        let model_fast = std::env::var("VIV_MODEL_FAST")
            .or_else(|_| fallback_model.clone().ok_or(std::env::VarError::NotPresent))
            .unwrap_or_else(|_| "claude-haiku-4-5".into());

        let model_medium = std::env::var("VIV_MODEL_MEDIUM")
            .or_else(|_| fallback_model.clone().ok_or(std::env::VarError::NotPresent))
            .unwrap_or_else(|_| "claude-sonnet-4-6".into());

        let model_slow = std::env::var("VIV_MODEL_SLOW")
            .or_else(|_| fallback_model.ok_or(std::env::VarError::NotPresent))
            .unwrap_or_else(|_| "claude-opus-4-6".into());

        Ok(LLMConfig {
            api_key,
            base_url,
            model_fast,
            model_medium,
            model_slow,
        })
    }

    /// Return the model string for the given tier.
    pub fn model(&self, tier: ModelTier) -> &str {
        match tier {
            ModelTier::Fast => &self.model_fast,
            ModelTier::Medium => &self.model_medium,
            ModelTier::Slow => &self.model_slow,
        }
    }

    /// Return the appropriate max_tokens value for the given tier.
    pub fn max_tokens(&self, tier: ModelTier) -> u64 {
        match tier {
            ModelTier::Fast => 8192,
            ModelTier::Medium => 64000,
            ModelTier::Slow => 128000,
        }
    }
}

/// Client for the Anthropic Claude API.
pub struct LLMClient {
    pub config: LLMConfig,
}

impl LLMClient {
    /// Create a new `LLMClient` with the given configuration.
    pub fn new(config: LLMConfig) -> Self {
        LLMClient { config }
    }

    /// Build the HTTP request for a streaming Claude API call.
    pub fn build_request(&self, messages: &[Message], tier: ModelTier) -> HttpRequest {
        let model = self.config.model(tier.clone()).to_string();
        let max_tokens = self.config.max_tokens(tier);
        let url = parse_base_url(&self.config.base_url);

        // Build the messages JSON array
        let messages_json: Vec<String> = messages
            .iter()
            .map(|m| {
                format!(
                    "{{\"role\":{},\"content\":{}}}",
                    JsonValue::Str(m.role.clone()),
                    JsonValue::Str(m.content.clone()),
                )
            })
            .collect();

        let body = format!(
            "{{\"model\":{},\"max_tokens\":{},\"stream\":true,\"messages\":[{}]}}",
            JsonValue::Str(model),
            max_tokens,
            messages_json.join(","),
        );

        HttpRequest {
            method: "POST".into(),
            path: format!("{}/v1/messages", url.path_prefix),
            headers: vec![
                ("Host".into(), url.host),
                ("Content-Type".into(), "application/json".into()),
                ("x-api-key".into(), self.config.api_key.clone()),
                ("anthropic-version".into(), "2023-06-01".into()),
            ],
            body: Some(body),
        }
    }

    /// Send a streaming request to the Claude API, calling `on_text` for each text delta.
    /// Returns the full accumulated response text.
    pub fn stream(
        &self,
        messages: &[Message],
        tier: ModelTier,
        mut on_text: impl FnMut(&str),
    ) -> crate::Result<String> {
        let req = self.build_request(messages, tier);
        let bytes = req.to_bytes();
        let url = parse_base_url(&self.config.base_url);

        // Connect via TLS
        let mut tls = TlsStream::connect(&url.host, url.port)?;

        // Send request
        tls.write_all(&bytes)?;

        // Read until we have the full SSE body
        let mut raw: Vec<u8> = Vec::new();
        let mut tmp = [0u8; 4096];

        let mut header_end: Option<usize> = None;
        loop {
            let n = tls.read(&mut tmp)?;
            if n == 0 {
                break;
            }
            raw.extend_from_slice(&tmp[..n]);

            // Check if we have the header separator
            if header_end.is_none() {
                if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = Some(pos + 4);

                    // Parse status from the header
                    let header_section = std::str::from_utf8(&raw[..pos])
                        .map_err(|_| Error::Http("invalid UTF-8 in headers".into()))?;
                    let status = parse_status_line(header_section)?;
                    if status != 200 {
                        // Read more to get the error body
                        loop {
                            let n2 = tls.read(&mut tmp)?;
                            if n2 == 0 { break; }
                            raw.extend_from_slice(&tmp[..n2]);
                        }
                        let body_bytes = &raw[pos + 4..];
                        let body_str = String::from_utf8_lossy(body_bytes).into_owned();
                        return Err(Error::LLM { status, message: body_str });
                    }
                }
            }

            // Once we have the header, process SSE body incrementally
            if let Some(hend) = header_end {
                let body_bytes = &raw[hend..];
                let body_str = String::from_utf8_lossy(body_bytes);

                if body_str.contains("message_stop") {
                    break;
                }
            }
        }

        // Parse the entire SSE body
        let mut accumulated = String::new();

        if let Some(hend) = header_end {
            let body_bytes = &raw[hend..];
            let body_str = String::from_utf8_lossy(body_bytes);

            let mut parser = SseParser::new();
            parser.feed(&body_str);
            let events = parser.drain();

            for event in events {
                match event.event.as_deref() {
                    Some("content_block_delta") | None => {
                        if let Some(text) = extract_delta_text(&event.data) {
                            on_text(&text);
                            accumulated.push_str(&text);
                        }
                    }
                    Some("message_stop") => {
                        break;
                    }
                    _ => {}
                }
            }
        }

        Ok(accumulated)
    }
}

/// Extract text from an SSE `content_block_delta` event's JSON data.
/// Returns `Some(text)` only for `text_delta` type deltas.
pub fn extract_delta_text(data: &str) -> Option<String> {
    let json = JsonValue::parse(data).ok()?;

    // Check top-level type is "content_block_delta"
    let top_type = json.get("type")?.as_str()?;
    if top_type != "content_block_delta" {
        return None;
    }

    let delta = json.get("delta")?;
    let delta_type = delta.get("type")?.as_str()?;
    if delta_type != "text_delta" {
        return None;
    }

    let text = delta.get("text")?.as_str()?;
    Some(text.to_string())
}

// ---- agent stream -----------------------------------------------------------
use crate::agent::message::ToJson;

/// 一次 LLM 流响应的完整结果
pub struct StreamResult {
    pub text_blocks: Vec<crate::agent::message::ContentBlock>,
    pub tool_uses: Vec<crate::agent::message::ContentBlock>,
    pub stop_reason: String,
}

impl LLMClient {
    /// 支持 tool_use 的流式请求：解析 text_delta 和 input_json_delta。
    /// system_blocks 对应 Anthropic API system 数组（带 cache_control）。
    pub fn stream_agent(
        &self,
        system_blocks: &[crate::agent::message::SystemBlock],
        messages: &[crate::agent::message::Message],
        tier: ModelTier,
        mut on_text: impl FnMut(&str),
    ) -> crate::Result<StreamResult> {
        let req = self.build_agent_request(system_blocks, messages, tier);
        let bytes = req.to_bytes();
        let url = parse_base_url(&self.config.base_url);

        let mut tls = TlsStream::connect(&url.host, url.port)?;
        tls.write_all(&bytes)?;

        let mut raw: Vec<u8> = Vec::new();
        let mut tmp = [0u8; 4096];
        let mut header_end: Option<usize> = None;

        loop {
            let n = tls.read(&mut tmp)?;
            if n == 0 { break; }
            raw.extend_from_slice(&tmp[..n]);

            if header_end.is_none() {
                if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = Some(pos + 4);
                    let header_section = std::str::from_utf8(&raw[..pos])
                        .map_err(|_| Error::Http("invalid UTF-8 in headers".into()))?;
                    let status = parse_status_line(header_section)?;
                    if status != 200 {
                        loop {
                            let n2 = tls.read(&mut tmp)?;
                            if n2 == 0 { break; }
                            raw.extend_from_slice(&tmp[..n2]);
                        }
                        let body = String::from_utf8_lossy(&raw[pos + 4..]).into_owned();
                        return Err(Error::LLM { status, message: body });
                    }
                }
            }
            if let Some(hend) = header_end {
                if String::from_utf8_lossy(&raw[hend..]).contains("message_stop") {
                    break;
                }
            }
        }

        parse_agent_stream(&raw, header_end, &mut on_text)
    }

    fn build_agent_request(
        &self,
        system_blocks: &[crate::agent::message::SystemBlock],
        messages: &[crate::agent::message::Message],
        tier: ModelTier,
    ) -> HttpRequest {
        let model = self.config.model(tier.clone()).to_string();
        let max_tokens = self.config.max_tokens(tier);
        let url = parse_base_url(&self.config.base_url);

        let system_json: Vec<String> = system_blocks.iter().map(|b| b.to_json()).collect();
        let messages_json: Vec<String> = messages.iter().map(|m| m.to_json()).collect();

        let body = format!(
            "{{\"model\":{},\"max_tokens\":{},\"stream\":true,\"system\":[{}],\"messages\":[{}]}}",
            JsonValue::Str(model),
            max_tokens,
            system_json.join(","),
            messages_json.join(","),
        );

        HttpRequest {
            method: "POST".into(),
            path: format!("{}/v1/messages", url.path_prefix),
            headers: vec![
                ("Host".into(), url.host),
                ("Content-Type".into(), "application/json".into()),
                ("x-api-key".into(), self.config.api_key.clone()),
                ("anthropic-version".into(), "2023-06-01".into()),
                ("anthropic-beta".into(), "prompt-caching-2024-07-31".into()),
            ],
            body: Some(body),
        }
    }
}

fn parse_agent_stream(
    raw: &[u8],
    header_end: Option<usize>,
    on_text: &mut impl FnMut(&str),
) -> crate::Result<StreamResult> {
    use crate::agent::message::ContentBlock;

    let mut text_acc: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
    let mut tool_acc: std::collections::HashMap<usize, (String, String, String)> =
        std::collections::HashMap::new();

    let mut text_blocks: Vec<ContentBlock> = vec![];
    let mut tool_uses: Vec<ContentBlock> = vec![];
    let mut stop_reason = String::from("end_turn");

    let hend = match header_end {
        Some(h) => h,
        None => return Ok(StreamResult { text_blocks, tool_uses, stop_reason }),
    };
    let body_str = String::from_utf8_lossy(&raw[hend..]);

    let mut parser = SseParser::new();
    parser.feed(&body_str);
    let events = parser.drain();

    for event in events {
        let data = &event.data;
        let json = match JsonValue::parse(data) { Ok(j) => j, Err(_) => continue };
        let ev_type = match json.get("type").and_then(|v| v.as_str()) { Some(t) => t, None => continue };

        match ev_type {
            "content_block_start" => {
                let idx = json.get("index").and_then(|v| v.as_i64()).unwrap_or(0) as usize;
                let block = json.get("content_block").unwrap_or(&JsonValue::Null);
                let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match block_type {
                    "text" => { text_acc.insert(idx, String::new()); }
                    "tool_use" => {
                        let id = block.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        tool_acc.insert(idx, (id, name, String::new()));
                    }
                    _ => {}
                }
            }
            "content_block_delta" => {
                let idx = json.get("index").and_then(|v| v.as_i64()).unwrap_or(0) as usize;
                let delta = json.get("delta").unwrap_or(&JsonValue::Null);
                let delta_type = delta.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match delta_type {
                    "text_delta" => {
                        if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                            on_text(text);
                            if let Some(acc) = text_acc.get_mut(&idx) { acc.push_str(text); }
                        }
                    }
                    "input_json_delta" => {
                        if let Some(partial) = delta.get("partial_json").and_then(|v| v.as_str()) {
                            if let Some(entry) = tool_acc.get_mut(&idx) {
                                entry.2.push_str(partial);
                            }
                        }
                    }
                    _ => {}
                }
            }
            "content_block_stop" => {
                let idx = json.get("index").and_then(|v| v.as_i64()).unwrap_or(0) as usize;
                if let Some(text) = text_acc.remove(&idx) {
                    text_blocks.push(ContentBlock::Text(text));
                }
                if let Some((id, name, json_str)) = tool_acc.remove(&idx) {
                    let input = JsonValue::parse(&json_str).unwrap_or(JsonValue::Object(vec![]));
                    tool_uses.push(ContentBlock::ToolUse { id, name, input });
                }
            }
            "message_delta" => {
                if let Some(reason) = json.get("delta")
                    .and_then(|d| d.get("stop_reason"))
                    .and_then(|v| v.as_str())
                {
                    stop_reason = reason.to_string();
                }
            }
            _ => {}
        }
    }

    Ok(StreamResult { text_blocks, tool_uses, stop_reason })
}

/// 仅供测试使用的公开入口
pub fn parse_agent_stream_pub(
    raw: &[u8],
    header_end: Option<usize>,
    on_text: &mut impl FnMut(&str),
) -> crate::Result<StreamResult> {
    parse_agent_stream(raw, header_end, on_text)
}

// ---- helpers ----------------------------------------------------------------

fn parse_status_line(header_section: &str) -> crate::Result<u16> {
    let first_line = header_section.lines().next()
        .ok_or_else(|| Error::Http("empty response".into()))?;
    let mut parts = first_line.splitn(3, ' ');
    let _version = parts.next();
    let code_str = parts.next()
        .ok_or_else(|| Error::Http(format!("malformed status line: {first_line}")))?;
    code_str.parse::<u16>()
        .map_err(|_| Error::Http(format!("invalid status code: {code_str}")))
}
