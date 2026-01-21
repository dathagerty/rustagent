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

#[test]
fn test_format_request_with_system_message() {
    let client = AnthropicClient::new(
        "test-key".to_string(),
        "claude-sonnet-4".to_string(),
        4096,
    );

    let messages = vec![
        Message {
            role: Role::System,
            content: "You are a helpful assistant".to_string(),
        },
        Message {
            role: Role::User,
            content: "Hello".to_string(),
        },
    ];

    let request = client.format_request(&messages, &[]).unwrap();

    // Should have system field
    assert!(request.get("system").is_some());
    assert_eq!(
        request.get("system").unwrap().as_str().unwrap(),
        "You are a helpful assistant"
    );

    // Should not include system message in messages array
    let msgs = request.get("messages").unwrap().as_array().unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].get("role").unwrap().as_str().unwrap(), "user");
}

use rustagent::llm::anthropic::is_retryable_error;

#[test]
fn test_is_retryable_error() {
    // Test retryable errors
    assert!(is_retryable_error("rate limit exceeded"));
    assert!(is_retryable_error("connection timeout"));
    assert!(is_retryable_error("network error"));
    assert!(is_retryable_error("502 Bad Gateway"));
    assert!(is_retryable_error("503 Service Unavailable"));
    assert!(is_retryable_error("504 Gateway Timeout"));

    // Test non-retryable errors
    assert!(!is_retryable_error("invalid request"));
    assert!(!is_retryable_error("400 Bad Request"));
    assert!(!is_retryable_error("401 Unauthorized"));
}
