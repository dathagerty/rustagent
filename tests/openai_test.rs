use rustagent::llm::openai::OpenAiClient;
use rustagent::llm::{Message, ToolDefinition};

#[test]
fn test_openai_format_request_basic() {
    let client = OpenAiClient::new("test-key".to_string(), "gpt-4".to_string(), 4096);

    let messages = vec![Message::user("Hello")];

    let request = client.format_request(&messages, &[]).unwrap();

    assert_eq!(request["model"], "gpt-4");
    assert_eq!(request["messages"][0]["role"], "user");
    assert_eq!(request["messages"][0]["content"], "Hello");
}

#[test]
fn test_openai_format_request_with_system() {
    let client = OpenAiClient::new("test-key".to_string(), "gpt-4".to_string(), 4096);

    let messages = vec![Message::system("You are helpful"), Message::user("Hello")];

    let request = client.format_request(&messages, &[]).unwrap();

    assert_eq!(request["messages"][0]["role"], "system");
    assert_eq!(request["messages"][1]["role"], "user");
}

#[test]
fn test_openai_format_request_with_tools() {
    let client = OpenAiClient::new("test-key".to_string(), "gpt-4".to_string(), 4096);

    let messages = vec![Message::user("Read file.txt")];

    let tools = vec![ToolDefinition {
        name: "read_file".to_string(),
        description: "Read a file".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"}
            },
            "required": ["path"]
        }),
    }];

    let request = client.format_request(&messages, &tools).unwrap();

    assert!(request["tools"].is_array());
    assert_eq!(request["tools"][0]["type"], "function");
    assert_eq!(request["tools"][0]["function"]["name"], "read_file");
}

#[test]
fn test_openai_format_request_with_tool_result() {
    let client = OpenAiClient::new("test-key".to_string(), "gpt-4".to_string(), 4096);

    let messages = vec![
        Message::user("Read file.txt"),
        Message::assistant("I'll read that file"),
        Message::tool_result("call_123", "File contents here"),
    ];

    let request = client.format_request(&messages, &[]).unwrap();

    assert_eq!(request["messages"][2]["role"], "tool");
    assert_eq!(request["messages"][2]["tool_call_id"], "call_123");
    assert_eq!(request["messages"][2]["content"], "File contents here");
}
