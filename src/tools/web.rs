use crate::core::json::JsonValue;
use crate::core::net::http::HttpRequest;
use crate::core::net::tls::AsyncTlsStream;
use crate::core::runtime::AssertSend;
use crate::error::Error;
use crate::llm::{LLMClient, ModelTier};
use crate::tools::{PermissionLevel, Tool};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub struct WebFetchTool {
    pub llm: Arc<LLMClient>,
}
impl WebFetchTool {
    pub fn new(llm: Arc<LLMClient>) -> Self {
        WebFetchTool { llm }
    }
}

impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "WebFetch"
    }

    fn description(&self) -> &str {
        "Fetches content from a specified URL and processes it using an AI model.\n\n- Takes a URL and a prompt as input\n- Fetches the URL content, converts HTML to Markdown\n- Processes the content with the prompt using a fast model\n- Returns the model's response about the content\n\nIMPORTANT: Will FAIL for authenticated or private URLs. The URL must be a fully-formed valid URL. HTTP URLs are automatically upgraded to HTTPS."
    }

    fn input_schema(&self) -> JsonValue {
        crate::tools::parse_schema(r#"{
            "type":"object",
            "properties":{
                "url":{"type":"string","description":"The URL to fetch content from"},
                "prompt":{"type":"string","description":"The prompt to run on the fetched content. Describe what information you want to extract from the page."}
            },
            "required":["url","prompt"]
        }"#)
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let input = input.clone();
        let llm = Arc::clone(&self.llm);
        Box::pin(AssertSend(async move {
            let url = input
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool("missing 'url'".into()))?;
            let prompt = input
                .get("prompt")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool("missing 'prompt'".into()))?;

            let text = fetch_url_async(url).await?;
            let truncated: String = text.chars().take(16000).collect();

            use crate::agent::message::{Message, SystemBlock};
            let system = vec![SystemBlock::dynamic(
                "You extract relevant content from web pages.",
            )];
            let user_msg = format!(
                "Answer this about the page: {}\n\nPage:\n{}",
                prompt, truncated
            );
            let messages = vec![Message::user_text(user_msg)];
            let mut response = String::new();
            llm.stream_agent_async(&system, &messages, "", ModelTier::Fast, |t| {
                response.push_str(t)
            })
            .await?;
            Ok(response)
        }))
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Execute
    }
}

async fn fetch_url_async(url: &str) -> crate::Result<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let (host_port, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match host_port.rfind(':') {
        Some(i) => (
            &host_port[..i],
            host_port[i + 1..].parse::<u16>().unwrap_or(443),
        ),
        None => (host_port, 443),
    };

    let req = HttpRequest {
        method: "GET".into(),
        path: path.to_string(),
        headers: vec![
            ("Host".into(), host.to_string()),
            ("User-Agent".into(), "viv/0.1".into()),
            ("Accept".into(), "text/html,text/plain".into()),
            ("Connection".into(), "close".into()),
        ],
        body: None,
    };

    let mut tls = AsyncTlsStream::connect(host, port).await?;
    tls.write_all(&req.to_bytes()).await?;

    let mut raw: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        let n = tls.read(&mut tmp).await?;
        if n == 0 || raw.len() > 1_000_000 {
            break;
        }
        raw.extend_from_slice(&tmp[..n]);
    }

    let body = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| &raw[i + 4..])
        .unwrap_or(&raw);

    Ok(html_to_markdown(&String::from_utf8_lossy(body)))
}

/// Extract the value of an attribute from a tag's inner content.
/// `tag` is the content between `<` and `>` (e.g. `a href="url" class="x"`).
fn extract_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let search = format!("{}=\"", name);
    if let Some(start) = tag.find(&search) {
        let val_start = start + search.len();
        if let Some(end) = tag[val_start..].find('"') {
            return Some(&tag[val_start..val_start + end]);
        }
    }
    // Also handle single-quoted attributes
    let search_sq = format!("{}='", name);
    if let Some(start) = tag.find(&search_sq) {
        let val_start = start + search_sq.len();
        if let Some(end) = tag[val_start..].find('\'') {
            return Some(&tag[val_start..val_start + end]);
        }
    }
    None
}

/// Strip all HTML tags, preserving text content only.
fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if in_tag => {}
            _ => out.push(ch),
        }
    }
    out
}

/// Decode common HTML entities.
fn decode_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&nbsp;", " ")
}

/// Collapse runs of whitespace (spaces/tabs) into a single space.
fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_space = false;
    for ch in text.chars() {
        if ch == ' ' || ch == '\t' {
            if !last_space {
                out.push(' ');
                last_space = true;
            }
        } else {
            out.push(ch);
            last_space = false;
        }
    }
    out
}

/// Convert HTML to Markdown.
///
/// Single-pass parser that tracks position, recognizes tags, and dispatches
/// to handlers. Handles headings, links, emphasis, code, pre blocks, lists,
/// line breaks, script/style removal, and HTML entity decoding.
pub fn html_to_markdown(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let chars: Vec<char> = html.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_pre = false;

    while i < len {
        if chars[i] == '<' {
            // Find the end of the tag
            let mut j = i + 1;
            while j < len && chars[j] != '>' {
                j += 1;
            }
            if j >= len {
                // Malformed — emit '<' and move on
                out.push('<');
                i += 1;
                continue;
            }
            let tag_content: String = chars[i + 1..j].iter().collect();
            let tag_lower = tag_content.to_ascii_lowercase();
            let tag_lower = tag_lower.trim();
            i = j + 1; // skip past '>'

            // Closing tags
            if let Some(tag_name) = tag_lower.strip_prefix('/') {
                let tag_name = tag_name.trim();
                match tag_name {
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        out.push('\n');
                    }
                    "p" => {
                        out.push_str("\n\n");
                    }
                    "strong" | "b" => {
                        out.push_str("**");
                    }
                    "em" | "i" => {
                        out.push('*');
                    }
                    "code" if !in_pre => {
                        out.push('`');
                    }
                    "pre" => {
                        out.push_str("\n```\n");
                        in_pre = false;
                    }
                    "li" => {
                        out.push('\n');
                    }
                    "ul" | "ol" => {
                        out.push('\n');
                    }
                    "a" => {
                        // Handled when we see the opening <a> tag
                    }
                    _ => {}
                }
                continue;
            }

            // Self-closing or opening tags
            let tag_name = tag_lower
                .split(|c: char| c.is_whitespace() || c == '/')
                .next()
                .unwrap_or("");

            match tag_name {
                "br" => {
                    out.push('\n');
                }
                "h1" => out.push_str("\n# "),
                "h2" => out.push_str("\n## "),
                "h3" => out.push_str("\n### "),
                "h4" => out.push_str("\n#### "),
                "h5" => out.push_str("\n##### "),
                "h6" => out.push_str("\n###### "),
                "p" => {
                    out.push_str("\n\n");
                }
                "strong" | "b" => {
                    out.push_str("**");
                }
                "em" | "i" => {
                    out.push('*');
                }
                "code" if !in_pre => {
                    out.push('`');
                }
                "pre" => {
                    out.push_str("\n```\n");
                    in_pre = true;
                }
                "li" => {
                    out.push_str("- ");
                }
                "script" | "style" => {
                    // Skip everything until the closing tag
                    let close_tag = format!("</{}", tag_name);
                    let rest: String = chars[i..].iter().collect();
                    let rest_lower = rest.to_ascii_lowercase();
                    if let Some(pos) = rest_lower.find(&close_tag) {
                        // Skip past the closing tag's '>'
                        let after_close = &rest[pos..];
                        let close_end = after_close.find('>').unwrap_or(after_close.len() - 1);
                        i += pos + close_end + 1;
                    }
                }
                "a" => {
                    // Extract href and look ahead for inner text + </a>
                    let href = extract_attr(&tag_content, "href").unwrap_or("");
                    let rest: String = chars[i..].iter().collect();
                    let rest_lower = rest.to_ascii_lowercase();
                    if let Some(close_pos) = rest_lower.find("</a>") {
                        let inner_html: String = chars[i..i + close_pos].iter().collect();
                        let inner_text = strip_tags(&inner_html);
                        let inner_text = decode_entities(&inner_text);
                        let inner_text = collapse_whitespace(inner_text.trim());
                        out.push('[');
                        out.push_str(&inner_text);
                        out.push_str("](");
                        out.push_str(href);
                        out.push(')');
                        // Skip past </a>
                        i += close_pos + 4; // len of "</a>"
                        // Skip past '>' of </a> — already included in the 4 chars
                        // Actually </a> is 4 chars but we need to find '>'
                        // The close_pos points to '<' of </a>, so +4 lands after 'a'
                        // but we need to also skip '>'
                        if i < len && chars[i] == '>' {
                            i += 1;
                        }
                    } else {
                        // No closing tag found, just output as text
                    }
                }
                "ul" | "ol" => {
                    out.push('\n');
                }
                _ => {
                    // Unknown tag — strip it (content preserved on its own)
                }
            }
        } else {
            // Regular text
            let ch = chars[i];
            if in_pre {
                out.push(ch);
            } else if ch == '&' {
                // Try to decode an entity
                let rest: String = chars[i..].iter().take(10).collect();
                if rest.starts_with("&amp;") {
                    out.push('&');
                    i += 5;
                    continue;
                } else if rest.starts_with("&lt;") {
                    out.push('<');
                    i += 4;
                    continue;
                } else if rest.starts_with("&gt;") {
                    out.push('>');
                    i += 4;
                    continue;
                } else if rest.starts_with("&quot;") {
                    out.push('"');
                    i += 6;
                    continue;
                } else if rest.starts_with("&nbsp;") {
                    out.push(' ');
                    i += 6;
                    continue;
                } else {
                    out.push('&');
                }
            } else if ch == ' ' || ch == '\t' {
                // Collapse whitespace outside pre
                if !out.ends_with(' ') && !out.ends_with('\n') {
                    out.push(' ');
                }
            } else if ch == '\n' || ch == '\r' {
                // Collapse newlines outside pre to single space
                if !out.ends_with(' ') && !out.ends_with('\n') {
                    out.push(' ');
                }
            } else {
                out.push(ch);
            }
            i += 1;
        }
    }

    // Clean up: collapse multiple blank lines into at most two newlines
    let mut result = String::with_capacity(out.len());
    let mut newline_count = 0;
    for ch in out.chars() {
        if ch == '\n' {
            newline_count += 1;
            if newline_count <= 2 {
                result.push(ch);
            }
        } else {
            newline_count = 0;
            result.push(ch);
        }
    }

    result.trim().to_string()
}
