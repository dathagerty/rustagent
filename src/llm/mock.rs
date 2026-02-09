use super::{LlmClient, Message, Response, ResponseContent, ToolCall, ToolDefinition};
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

type RecordedCalls = Vec<(Vec<Message>, Vec<ToolDefinition>)>;
type MockResponseQueue = VecDeque<(ResponseContent, Option<String>)>;

pub struct MockLlmClient {
    responses: Arc<Mutex<MockResponseQueue>>,
    recorded_calls: Arc<Mutex<RecordedCalls>>,
    token_counts: Arc<Mutex<Option<(usize, usize)>>>, // (input_tokens, output_tokens)
}

impl MockLlmClient {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::new())),
            recorded_calls: Arc::new(Mutex::new(Vec::new())),
            token_counts: Arc::new(Mutex::new(None)),
        }
    }

    pub fn queue_text_response(&self, text: &str) {
        self.responses.lock().unwrap().push_back((
            ResponseContent::Text(text.to_string()),
            Some("end_turn".to_string()),
        ));
    }

    pub fn queue_tool_call(&self, name: &str, params: serde_json::Value) {
        self.responses.lock().unwrap().push_back((
            ResponseContent::ToolCalls(vec![ToolCall {
                id: format!("call_{}", uuid::Uuid::new_v4()),
                name: name.to_string(),
                parameters: params,
            }]),
            Some("tool_use".to_string()),
        ));
    }

    pub fn set_token_counts(&self, input: usize, output: usize) {
        *self.token_counts.lock().unwrap() = Some((input, output));
    }

    pub fn get_recorded_calls(&self) -> Vec<(Vec<Message>, Vec<ToolDefinition>)> {
        self.recorded_calls.lock().unwrap().clone()
    }
}

impl Default for MockLlmClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn chat(
        &self,
        messages: Vec<Message>,
        tools: &[ToolDefinition],
    ) -> anyhow::Result<Response> {
        self.recorded_calls
            .lock()
            .unwrap()
            .push((messages, tools.to_vec()));

        let (content, stop_reason) = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("No more mock responses queued"))?;

        let token_counts = *self.token_counts.lock().unwrap();

        Ok(Response {
            content,
            stop_reason,
            input_tokens: token_counts.map(|(i, _)| i),
            output_tokens: token_counts.map(|(_, o)| o),
        })
    }
}
