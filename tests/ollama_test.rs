use rustagent::llm::ollama::OllamaClient;
use rustagent::llm::{Message, ToolDefinition};

#[test]
fn test_ollama_format_request_basic() {
    let client = OllamaClient::new("http://localhost:11434".to_string(), "llama3".to_string());

    let messages = vec![Message::user("Hello")];

    let request = client.format_request(&messages, &[]).unwrap();

    assert_eq!(request["model"], "llama3");
    assert_eq!(request["messages"][0]["role"], "user");
    assert_eq!(request["stream"], false);
}

#[test]
fn test_ollama_format_request_with_tools() {
    let client = OllamaClient::new("http://localhost:11434".to_string(), "llama3".to_string());

    let messages = vec![Message::user("Read file")];

    let tools = vec![ToolDefinition {
        name: "read_file".to_string(),
        description: "Read a file".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"}
            }
        }),
    }];

    let request = client.format_request(&messages, &tools).unwrap();

    assert!(request["tools"].is_array());
    assert_eq!(request["tools"][0]["function"]["name"], "read_file");
}

#[test]
fn test_ollama_format_request_with_system() {
    let client = OllamaClient::new("http://localhost:11434".to_string(), "llama3".to_string());

    let messages = vec![Message::system("You are helpful"), Message::user("Hello")];

    let request = client.format_request(&messages, &[]).unwrap();

    assert_eq!(request["messages"][0]["role"], "system");
    assert_eq!(request["messages"][1]["role"], "user");
}
