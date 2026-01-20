use rustagent::tools::{Tool, ToolRegistry};
use async_trait::async_trait;
use anyhow::Result;

struct MockTool;

#[async_trait]
impl Tool for MockTool {
    fn name(&self) -> &str {
        "mock_tool"
    }

    fn description(&self) -> &str {
        "A mock tool"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _params: serde_json::Value) -> Result<String> {
        Ok("success".to_string())
    }
}

#[tokio::test]
async fn test_tool_registry() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(MockTool));

    assert!(registry.get("mock_tool").is_some());
    assert!(registry.get("nonexistent").is_none());
}

#[tokio::test]
async fn test_tool_execute() {
    let tool = MockTool;
    let result = tool.execute(serde_json::json!({})).await.unwrap();
    assert_eq!(result, "success");
}
