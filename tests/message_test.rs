use rustagent::llm::{Message, Role};

#[test]
fn test_tool_message_creation() {
    let msg = Message::tool_result("call_123", "File contents here");
    assert_eq!(msg.role, Role::Tool);
    assert_eq!(msg.tool_call_id, Some("call_123".to_string()));
    assert_eq!(msg.content, "File contents here");
}

#[test]
fn test_user_message_has_no_tool_call_id() {
    let msg = Message::user("Hello");
    assert_eq!(msg.role, Role::User);
    assert!(msg.tool_call_id.is_none());
}

#[test]
fn test_assistant_message_constructor() {
    let msg = Message::assistant("I can help with that");
    assert_eq!(msg.role, Role::Assistant);
    assert_eq!(msg.content, "I can help with that");
    assert!(msg.tool_call_id.is_none());
}

#[test]
fn test_system_message_constructor() {
    let msg = Message::system("You are a helpful assistant");
    assert_eq!(msg.role, Role::System);
    assert_eq!(msg.content, "You are a helpful assistant");
    assert!(msg.tool_call_id.is_none());
}

#[test]
fn test_tool_message_serialization() {
    let msg = Message::tool_result("call_abc", "result data");
    let json = serde_json::to_value(&msg).unwrap();

    assert_eq!(json["role"], "tool");
    assert_eq!(json["tool_call_id"], "call_abc");
    assert_eq!(json["content"], "result data");
}

#[test]
fn test_user_message_serialization_skips_none_tool_call_id() {
    let msg = Message::user("Hello");
    let json = serde_json::to_value(&msg).unwrap();

    assert_eq!(json["role"], "user");
    assert_eq!(json["content"], "Hello");
    // tool_call_id should not be present (skip_serializing_if = "Option::is_none")
    assert!(json.get("tool_call_id").is_none());
}
