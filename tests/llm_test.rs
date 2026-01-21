use rustagent::llm::{Message, Role, ToolDefinition};
use serde_json::json;

#[test]
fn test_message_serialization() {
    let msg = Message {
        role: Role::User,
        content: "Hello".to_string(),
    };

    let json = serde_json::to_value(&msg).unwrap();
    assert_eq!(json["role"], "user");
    assert_eq!(json["content"], "Hello");
}

#[test]
fn test_tool_definition_parameters() {
    let tool = ToolDefinition {
        name: "read_file".to_string(),
        description: "Read a file".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"}
            },
            "required": ["path"]
        }),
    };

    assert_eq!(tool.name, "read_file");
}

use rustagent::llm::anthropic::AnthropicClient;

#[tokio::test]
async fn test_anthropic_message_format() {
    // This test validates request structure, doesn't actually call API
    let client = AnthropicClient::new(
        "test-key".to_string(),
        "claude-sonnet-4".to_string(),
        4096,
    );

    let messages = vec![Message {
        role: Role::User,
        content: "Hello".to_string(),
    }];

    // We'll test this by mocking in future, for now just construct
    assert!(client.format_request(&messages, &[]).is_ok());
}
