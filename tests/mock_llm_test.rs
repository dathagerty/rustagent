use rustagent::llm::mock::MockLlmClient;
use rustagent::llm::{LlmClient, Message, ResponseContent};

#[tokio::test]
async fn test_mock_text_response() {
    let client = MockLlmClient::new();
    client.queue_text_response("Hello, world!");

    let messages = vec![Message::user("Hi")];

    let response = client.chat(messages, &[]).await.unwrap();

    match response.content {
        ResponseContent::Text(text) => assert_eq!(text, "Hello, world!"),
        _ => panic!("Expected text response"),
    }
}

#[tokio::test]
async fn test_mock_tool_call_response() {
    let client = MockLlmClient::new();
    client.queue_tool_call("read_file", serde_json::json!({"path": "test.txt"}));

    let messages = vec![Message::user("Read the file")];

    let response = client.chat(messages, &[]).await.unwrap();

    match response.content {
        ResponseContent::ToolCalls(calls) => {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].name, "read_file");
        }
        _ => panic!("Expected tool call response"),
    }
}

#[tokio::test]
async fn test_mock_records_calls() {
    let client = MockLlmClient::new();
    client.queue_text_response("Response 1");
    client.queue_text_response("Response 2");

    let msg1 = vec![Message::user("First")];
    let msg2 = vec![Message::user("Second")];

    client.chat(msg1, &[]).await.unwrap();
    client.chat(msg2, &[]).await.unwrap();

    let calls = client.get_recorded_calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0[0].content, "First");
    assert_eq!(calls[1].0[0].content, "Second");
}

#[tokio::test]
async fn test_mock_no_response_error() {
    let client = MockLlmClient::new();

    let messages = vec![Message::user("Hi")];
    let result = client.chat(messages, &[]).await;

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("No more mock responses")
    );
}
