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
pub struct LlmConfig {
    pub api_key: String,
    pub base_url: String,
    pub model_fast: String,
    pub model_medium: String,
    pub model_slow: String,
}

impl LlmConfig {
    /// Build an `LlmConfig` from environment variables.
    ///
    /// - `VIV_API_KEY` — required
    /// - `VIV_BASE_URL` — optional (default: "api.anthropic.com")
    /// - `VIV_MODEL_FAST` — optional, falls back to `VIV_MODEL`, then default
    /// - `VIV_MODEL_MEDIUM` — optional, falls back to `VIV_MODEL`, then default
    /// - `VIV_MODEL_SLOW` — optional, falls back to `VIV_MODEL`, then default
    /// - `VIV_MODEL` — optional fallback for all three tiers
    pub fn from_env() -> crate::Result<Self> {
        let api_key = std::env::var("VIV_API_KEY").map_err(|_| Error::Api {
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

        Ok(LlmConfig {
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
pub struct LlmClient {
    pub config: LlmConfig,
}

impl LlmClient {
    /// Create a new `LlmClient` with the given configuration.
    pub fn new(config: LlmConfig) -> Self {
        LlmClient { config }
    }

    /// Build the HTTP request for a streaming Claude API call.
    pub fn build_request(&self, messages: &[Message], tier: ModelTier) -> HttpRequest {
        let model = self.config.model(tier.clone()).to_string();
        let max_tokens = self.config.max_tokens(tier);

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
            path: "/v1/messages".into(),
            headers: vec![
                ("Host".into(), self.config.base_url.clone()),
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

        // Connect via TLS
        let mut tls = TlsStream::connect(&self.config.base_url, 443)?;

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
                        return Err(Error::Api { status, message: body_str });
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
