use rustagent::tools::Tool;
use rustagent::tools::signal::SignalTool;

#[tokio::test]
async fn test_signal_complete() {
    let tool = SignalTool::new();

    let params = serde_json::json!({
        "signal": "complete",
        "message": "Task finished successfully"
    });

    let result = tool.execute(params).await.unwrap();
    assert!(result.contains("complete"));
}

#[tokio::test]
async fn test_signal_blocked() {
    let tool = SignalTool::new();

    let params = serde_json::json!({
        "signal": "blocked",
        "reason": "Missing dependency"
    });

    let result = tool.execute(params).await.unwrap();
    assert!(result.contains("blocked"));
}

#[test]
fn test_signal_tool_parameters() {
    let tool = SignalTool::new();
    let params = tool.parameters();

    assert!(params["properties"]["signal"].is_object());
    assert!(params["properties"]["message"].is_object());
    assert!(params["properties"]["reason"].is_object());
}
