use viv::core::json::JsonValue;
use viv::tools::Tool;
use viv::tools::poll_to_completion;
use viv::tools::search::WebSearchTool;

#[test]
fn search_without_api_key_returns_friendly_error() {
    // SAFETY: No other threads are reading VIV_TAVILY_API_KEY concurrently in this test.
    unsafe { std::env::remove_var("VIV_TAVILY_API_KEY") };
    let tool = WebSearchTool;
    let input = JsonValue::parse(r#"{"query":"rust programming"}"#).unwrap();
    let result = poll_to_completion(tool.execute(&input));
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("VIV_TAVILY_API_KEY"),
        "Error should mention env var: {}",
        err
    );
}

#[test]
fn search_tool_has_correct_name_and_permission() {
    let tool = WebSearchTool;
    assert_eq!(tool.name(), "WebSearch");
    assert_eq!(
        tool.permission_level(),
        viv::tools::PermissionLevel::ReadOnly
    );
}

#[test]
fn search_missing_query_returns_error() {
    let tool = WebSearchTool;
    let input = JsonValue::parse(r#"{"max_results":5}"#).unwrap();
    let result = poll_to_completion(tool.execute(&input));
    assert!(result.is_err());
}
