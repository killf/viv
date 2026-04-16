use viv::llm::*;
use viv::json::JsonValue;

#[test]
fn build_api_request_with_tier() {
    let config = LlmConfig {
        api_key: "test-key".into(),
        base_url: "api.anthropic.com".into(),
        model_fast: "claude-haiku-4-5".into(),
        model_medium: "claude-sonnet-4-6".into(),
        model_slow: "claude-opus-4-6".into(),
    };
    let client = LlmClient::new(config);
    let messages = vec![Message { role: "user".into(), content: "Hello".into() }];
    let req = client.build_request(&messages, ModelTier::Slow);
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, "/v1/messages");
    let has_api_key = req.headers.iter().any(|(k, v)| k == "x-api-key" && v == "test-key");
    assert!(has_api_key);
    let body = req.body.as_ref().unwrap();
    let json = JsonValue::parse(body).unwrap();
    assert_eq!(json.get("model").and_then(|v| v.as_str()), Some("claude-opus-4-6"));
    assert_eq!(json.get("max_tokens").and_then(|v| v.as_f64()), Some(128000.0));
    assert_eq!(json.get("stream").and_then(|v| v.as_bool()), Some(true));
}

#[test]
fn build_request_fast_tier() {
    let config = LlmConfig {
        api_key: "k".into(),
        base_url: "api.anthropic.com".into(),
        model_fast: "claude-haiku-4-5".into(),
        model_medium: "claude-sonnet-4-6".into(),
        model_slow: "claude-opus-4-6".into(),
    };
    let client = LlmClient::new(config);
    let req = client.build_request(&[Message { role: "user".into(), content: "hi".into() }], ModelTier::Fast);
    let json = JsonValue::parse(req.body.as_ref().unwrap()).unwrap();
    assert_eq!(json.get("model").and_then(|v| v.as_str()), Some("claude-haiku-4-5"));
    assert_eq!(json.get("max_tokens").and_then(|v| v.as_f64()), Some(8192.0));
}

#[test]
fn extract_text_from_delta() {
    let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello world"}}"#;
    assert_eq!(extract_delta_text(data), Some("Hello world".into()));
}

#[test]
fn extract_text_from_non_delta() {
    assert_eq!(extract_delta_text(r#"{"type":"message_start"}"#), None);
}

#[test]
fn extract_text_from_thinking_delta() {
    let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hmm"}}"#;
    assert_eq!(extract_delta_text(data), None);
}

#[test]
fn config_from_env() {
    let prev_key = std::env::var("VIV_API_KEY").ok();
    let prev_url = std::env::var("VIV_BASE_URL").ok();
    unsafe {
        std::env::set_var("VIV_API_KEY", "test-viv-key");
        std::env::set_var("VIV_BASE_URL", "custom.api.com");
    }
    let config = LlmConfig::from_env().unwrap();
    assert_eq!(config.api_key, "test-viv-key");
    assert_eq!(config.base_url, "custom.api.com");
    unsafe {
        match prev_key { Some(v) => std::env::set_var("VIV_API_KEY", v), None => std::env::remove_var("VIV_API_KEY") }
        match prev_url { Some(v) => std::env::set_var("VIV_BASE_URL", v), None => std::env::remove_var("VIV_BASE_URL") }
    }
}

#[test]
fn config_missing_api_key() {
    let prev = std::env::var("VIV_API_KEY").ok();
    unsafe { std::env::remove_var("VIV_API_KEY"); }
    assert!(LlmConfig::from_env().is_err());
    if let Some(v) = prev { unsafe { std::env::set_var("VIV_API_KEY", v); } }
}

#[test]
fn model_tier_selection() {
    let config = LlmConfig {
        api_key: "k".into(), base_url: "x".into(),
        model_fast: "fast".into(), model_medium: "med".into(), model_slow: "slow".into(),
    };
    assert_eq!(config.model(ModelTier::Fast), "fast");
    assert_eq!(config.model(ModelTier::Medium), "med");
    assert_eq!(config.model(ModelTier::Slow), "slow");
}

#[test]
fn max_tokens_per_tier() {
    let config = LlmConfig {
        api_key: "k".into(), base_url: "x".into(),
        model_fast: "f".into(), model_medium: "m".into(), model_slow: "s".into(),
    };
    assert_eq!(config.max_tokens(ModelTier::Fast), 8192);
    assert_eq!(config.max_tokens(ModelTier::Medium), 64000);
    assert_eq!(config.max_tokens(ModelTier::Slow), 128000);
}

#[test]
fn config_viv_model_fallback() {
    // Save
    let prev_key = std::env::var("VIV_API_KEY").ok();
    let prev_model = std::env::var("VIV_MODEL").ok();
    let prev_fast = std::env::var("VIV_MODEL_FAST").ok();
    let prev_medium = std::env::var("VIV_MODEL_MEDIUM").ok();
    let prev_slow = std::env::var("VIV_MODEL_SLOW").ok();

    // Set VIV_MODEL as fallback, no tier-specific vars
    unsafe {
        std::env::set_var("VIV_API_KEY", "k");
        std::env::set_var("VIV_MODEL", "my-custom-model");
        std::env::remove_var("VIV_MODEL_FAST");
        std::env::remove_var("VIV_MODEL_MEDIUM");
        std::env::remove_var("VIV_MODEL_SLOW");
    }

    let config = LlmConfig::from_env().unwrap();
    assert_eq!(config.model_fast, "my-custom-model");
    assert_eq!(config.model_medium, "my-custom-model");
    assert_eq!(config.model_slow, "my-custom-model");

    // Tier-specific overrides VIV_MODEL
    unsafe { std::env::set_var("VIV_MODEL_FAST", "override-fast"); }
    let config2 = LlmConfig::from_env().unwrap();
    assert_eq!(config2.model_fast, "override-fast");
    assert_eq!(config2.model_medium, "my-custom-model");

    // Restore
    unsafe {
        match prev_key { Some(v) => std::env::set_var("VIV_API_KEY", v), None => std::env::remove_var("VIV_API_KEY") }
        match prev_model { Some(v) => std::env::set_var("VIV_MODEL", v), None => std::env::remove_var("VIV_MODEL") }
        match prev_fast { Some(v) => std::env::set_var("VIV_MODEL_FAST", v), None => std::env::remove_var("VIV_MODEL_FAST") }
        match prev_medium { Some(v) => std::env::set_var("VIV_MODEL_MEDIUM", v), None => std::env::remove_var("VIV_MODEL_MEDIUM") }
        match prev_slow { Some(v) => std::env::set_var("VIV_MODEL_SLOW", v), None => std::env::remove_var("VIV_MODEL_SLOW") }
    }
}

/// End-to-end test: actually calls the Claude API.
/// Only compiled when full feature is enabled (costs money!).
/// Run with: cargo test --features full
#[cfg(feature = "full")]
#[test]
fn e2e_stream_real_api() {
    let config = match LlmConfig::from_env() {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping e2e test: VIV_API_KEY not set");
            return;
        }
    };
    let client = LlmClient::new(config);
    let messages = vec![Message {
        role: "user".into(),
        content: "Reply with exactly one word: hello".into(),
    }];

    let mut received_text = false;
    let result = client.stream(&messages, ModelTier::Fast, |text| {
        assert!(!text.is_empty());
        received_text = true;
    });

    assert!(result.is_ok(), "API call failed: {:?}", result.err());
    assert!(received_text, "No text was streamed");
    let response = result.unwrap();
    assert!(!response.is_empty(), "Response was empty");
    println!("e2e response: {}", response);
}
