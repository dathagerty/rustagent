use rustagent::llm::{Message, ToolDefinition};
use serde_json::json;

#[test]
fn test_message_serialization() {
    let msg = Message::user("Hello");

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
    let client = AnthropicClient::new("test-key".to_string(), "claude-sonnet-4".to_string(), 4096);

    let messages = vec![Message::user("Hello")];

    // We'll test this by mocking in future, for now just construct
    assert!(client.format_request(&messages, &[]).is_ok());
}

#[test]
fn test_format_request_with_system_message() {
    let client = AnthropicClient::new("test-key".to_string(), "claude-sonnet-4".to_string(), 4096);

    let messages = vec![
        Message::system("You are a helpful assistant"),
        Message::user("Hello"),
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

use reqwest::StatusCode;
use rustagent::llm::error::{classify_status, ErrorKind};
use rustagent::llm::retry::parse_retry_from_message;

#[test]
fn test_classify_status_retryable() {
    assert_eq!(classify_status(StatusCode::TOO_MANY_REQUESTS), ErrorKind::RateLimited);
    assert_eq!(classify_status(StatusCode::BAD_GATEWAY), ErrorKind::Transient);
    assert_eq!(classify_status(StatusCode::SERVICE_UNAVAILABLE), ErrorKind::Transient);
    assert_eq!(classify_status(StatusCode::GATEWAY_TIMEOUT), ErrorKind::Transient);
}

#[test]
fn test_classify_status_non_retryable() {
    assert_eq!(classify_status(StatusCode::BAD_REQUEST), ErrorKind::BadRequest);
    assert_eq!(classify_status(StatusCode::UNAUTHORIZED), ErrorKind::Auth);
    assert_eq!(classify_status(StatusCode::FORBIDDEN), ErrorKind::Auth);
}

#[test]
fn test_parse_retry_from_message() {
    let msg = "Rate limit reached. Please try again in 45.622s.";
    let dur = parse_retry_from_message(msg).unwrap();
    assert!(dur.as_millis() >= 45000 && dur.as_millis() <= 46000);

    let msg2 = "Try again in 500ms";
    let dur2 = parse_retry_from_message(msg2).unwrap();
    assert_eq!(dur2.as_millis(), 500);

    assert!(parse_retry_from_message("Something went wrong").is_none());
}
