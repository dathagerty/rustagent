use rustagent::llm::mock::MockLlmClient;
use rustagent::llm::{LlmClient, Message, ResponseContent};

#[tokio::test]
async fn test_response_has_token_fields() {
    // Verify Response struct contains input_tokens and output_tokens fields
    let client = MockLlmClient::new();
    client.queue_text_response("Hello, world!");

    let messages = vec![Message::user("Hi")];
    let response = client.chat(messages, &[]).await.unwrap();

    // Fields should exist, though they may be None for mock responses
    assert_eq!(
        response.content,
        ResponseContent::Text("Hello, world!".to_string())
    );
    // These fields should exist on Response
    let _ = response.input_tokens;
    let _ = response.output_tokens;
}

#[tokio::test]
async fn test_mock_can_set_token_counts() {
    // Verify mock client supports setting token counts
    let client = MockLlmClient::new();
    client.queue_text_response("Hello, world!");
    client.set_token_counts(100, 50);

    let messages = vec![Message::user("Hi")];
    let response = client.chat(messages, &[]).await.unwrap();

    assert_eq!(response.input_tokens, Some(100));
    assert_eq!(response.output_tokens, Some(50));
}

#[tokio::test]
async fn test_mock_token_counts_default_none() {
    // Verify mock responses have None by default for token counts
    let client = MockLlmClient::new();
    client.queue_text_response("Hello");

    let messages = vec![Message::user("Hi")];
    let response = client.chat(messages, &[]).await.unwrap();

    // Default should be None (no token info set)
    assert_eq!(response.input_tokens, None);
    assert_eq!(response.output_tokens, None);
}

#[tokio::test]
async fn test_token_counts_with_tool_calls() {
    // Verify token counts work with tool call responses too
    let client = MockLlmClient::new();
    client.queue_tool_call("read_file", serde_json::json!({"path": "test.txt"}));
    client.set_token_counts(75, 25);

    let messages = vec![Message::user("Read the file")];
    let response = client.chat(messages, &[]).await.unwrap();

    assert_eq!(response.input_tokens, Some(75));
    assert_eq!(response.output_tokens, Some(25));
    match response.content {
        ResponseContent::ToolCalls(calls) => {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].name, "read_file");
        }
        _ => panic!("Expected tool call response"),
    }
}
