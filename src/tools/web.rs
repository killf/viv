use std::pin::Pin;
use std::future::Future;
use std::sync::Arc;
use crate::core::json::JsonValue;
use crate::core::net::tls::AsyncTlsStream;
use crate::core::net::http::HttpRequest;
use crate::core::runtime::AssertSend;
use crate::error::Error;
use crate::llm::{LLMClient, ModelTier};
use crate::tools::{PermissionLevel, Tool};

pub struct WebFetchTool { pub llm: Arc<LLMClient> }
impl WebFetchTool {
    pub fn new(llm: Arc<LLMClient>) -> Self { WebFetchTool { llm } }
}

impl Tool for WebFetchTool {
    fn name(&self) -> &str { "WebFetch" }

    fn description(&self) -> &str {
        "Fetches content from a specified URL and processes it using an AI model.\n\n- Takes a URL and a prompt as input\n- Fetches the URL content, converts HTML to plain text\n- Processes the content with the prompt using a fast model\n- Returns the model's response about the content\n\nIMPORTANT: Will FAIL for authenticated or private URLs. The URL must be a fully-formed valid URL. HTTP URLs are automatically upgraded to HTTPS."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "url":{"type":"string","description":"The URL to fetch content from"},
                "prompt":{"type":"string","description":"The prompt to run on the fetched content. Describe what information you want to extract from the page."}
            },
            "required":["url","prompt"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let input = input.clone();
        let llm = Arc::clone(&self.llm);
        Box::pin(AssertSend(async move {
            let url = input.get("url").and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool("missing 'url'".into()))?;
            let prompt = input.get("prompt").and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool("missing 'prompt'".into()))?;

            let text = fetch_url_async(url).await?;
            let truncated: String = text.chars().take(8000).collect();

            use crate::agent::message::{Message, SystemBlock};
            let system = vec![SystemBlock::dynamic("You extract relevant content from web pages.")];
            let user_msg = format!("Answer this about the page: {}\n\nPage:\n{}", prompt, truncated);
            let messages = vec![Message::user_text(user_msg)];
            let mut response = String::new();
            llm.stream_agent_async(&system, &messages, "", ModelTier::Fast, |t| response.push_str(t)).await?;
            Ok(response)
        }))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Execute }
}

async fn fetch_url_async(url: &str) -> crate::Result<String> {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).unwrap_or(url);
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
        if n == 0 || raw.len() > 1_000_000 { break; }
        raw.extend_from_slice(&tmp[..n]);
    }

    let body = raw.windows(4).position(|w| w == b"\r\n\r\n")
        .map(|i| &raw[i + 4..])
        .unwrap_or(&raw);

    Ok(strip_html(&String::from_utf8_lossy(body)))
}

fn strip_html(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let mut last_space = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if in_tag => {}
            ' ' | '\t' => { if !last_space { out.push(' '); last_space = true; } }
            '\n' | '\r' => { out.push('\n'); last_space = false; }
            _ => { out.push(ch); last_space = false; }
        }
    }
    out
}
