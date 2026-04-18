use crate::core::json::JsonValue;
use crate::core::net::http::HttpRequest;
use crate::core::net::tls::AsyncTlsStream;
use crate::core::runtime::AssertSend;
use crate::error::Error;
use crate::tools::{PermissionLevel, Tool};
use std::future::Future;
use std::pin::Pin;

pub struct WebSearchTool;

impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "WebSearch"
    }

    fn description(&self) -> &str {
        "Search the web using Tavily API. Returns relevant web content with titles, URLs, and summaries. Requires VIV_TAVILY_API_KEY environment variable."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(
            r#"{
            "type":"object",
            "properties":{
                "query":{"type":"string","description":"Search query keywords"},
                "max_results":{"type":"number","description":"Maximum number of results (default 10, max 20)"},
                "search_depth":{"type":"string","enum":["basic","advanced"],"description":"Search depth (default: basic)"},
                "topic":{"type":"string","enum":["general","news"],"description":"Search topic (default: general)"},
                "include_domains":{"type":"array","items":{"type":"string"},"description":"Only include results from these domains"},
                "exclude_domains":{"type":"array","items":{"type":"string"},"description":"Exclude results from these domains"}
            },
            "required":["query"]
        }"#,
        )
        .unwrap()
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let input = input.clone();
        Box::pin(AssertSend(async move {
            let api_key = std::env::var("VIV_TAVILY_API_KEY").map_err(|_| {
                Error::Tool(
                    "VIV_TAVILY_API_KEY environment variable not set. Please set it to use web search."
                        .into(),
                )
            })?;

            let query = input
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool("missing 'query' parameter".into()))?;

            let max_results = input
                .get("max_results")
                .and_then(|v| v.as_i64())
                .map(|n| n.clamp(1, 20) as u32)
                .unwrap_or(10);

            let search_depth = input
                .get("search_depth")
                .and_then(|v| v.as_str())
                .unwrap_or("basic");

            let topic = input
                .get("topic")
                .and_then(|v| v.as_str())
                .unwrap_or("general");

            // Build request body
            let mut body_pairs: Vec<(String, JsonValue)> = vec![
                ("api_key".into(), JsonValue::Str(api_key)),
                ("query".into(), JsonValue::Str(query.to_string())),
                (
                    "max_results".into(),
                    JsonValue::Number(crate::core::json::Number::Int(max_results as i64)),
                ),
                (
                    "search_depth".into(),
                    JsonValue::Str(search_depth.to_string()),
                ),
                ("topic".into(), JsonValue::Str(topic.to_string())),
            ];

            if let Some(domains) = input.get("include_domains") {
                if let Some(arr) = domains.as_array() {
                    if !arr.is_empty() {
                        body_pairs.push(("include_domains".into(), domains.clone()));
                    }
                }
            }

            if let Some(domains) = input.get("exclude_domains") {
                if let Some(arr) = domains.as_array() {
                    if !arr.is_empty() {
                        body_pairs.push(("exclude_domains".into(), domains.clone()));
                    }
                }
            }

            let body_json = JsonValue::Object(body_pairs);
            let body_str = format!("{}", body_json);

            let req = HttpRequest {
                method: "POST".into(),
                path: "/search".into(),
                headers: vec![
                    ("Host".into(), "api.tavily.com".into()),
                    ("Content-Type".into(), "application/json".into()),
                    ("User-Agent".into(), "viv/0.1".into()),
                    ("Accept".into(), "application/json".into()),
                    ("Connection".into(), "close".into()),
                ],
                body: Some(body_str),
            };

            let mut tls = AsyncTlsStream::connect("api.tavily.com", 443).await?;
            tls.write_all(&req.to_bytes()).await?;

            let mut raw: Vec<u8> = Vec::new();
            let mut tmp = [0u8; 4096];
            loop {
                let n = tls.read(&mut tmp).await?;
                if n == 0 || raw.len() > 2_000_000 {
                    break;
                }
                raw.extend_from_slice(&tmp[..n]);
            }

            // Extract body after \r\n\r\n
            let body = raw
                .windows(4)
                .position(|w| w == b"\r\n\r\n")
                .map(|i| &raw[i + 4..])
                .unwrap_or(&raw);

            let body_text = String::from_utf8_lossy(body);
            let response = JsonValue::parse(&body_text)
                .map_err(|e| Error::Tool(format!("failed to parse Tavily response: {}", e)))?;

            // Extract results
            let results = match response.get("results") {
                Some(JsonValue::Array(arr)) => arr,
                _ => return Ok("No results found.".to_string()),
            };

            if results.is_empty() {
                return Ok("No results found.".to_string());
            }

            let mut output = String::new();
            for (i, result) in results.iter().enumerate() {
                let title = result
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(no title)");
                let url = result
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(no url)");
                let content = result
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(no content)");

                output.push_str(&format!("{}. {}\n   {}\n   {}\n\n", i + 1, title, url, content));
            }

            Ok(output.trim_end().to_string())
        }))
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}
