use super::{LlmClient, Message, Response, ResponseContent, ToolCall, ToolDefinition};
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

type RecordedCalls = Vec<(Vec<Message>, Vec<ToolDefinition>)>;

pub struct MockLlmClient {
    responses: Arc<Mutex<VecDeque<Response>>>,
    recorded_calls: Arc<Mutex<RecordedCalls>>,
}

impl MockLlmClient {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::new())),
            recorded_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn queue_text_response(&self, text: &str) {
        let response = Response {
            content: ResponseContent::Text(text.to_string()),
            stop_reason: Some("end_turn".to_string()),
        };
        self.responses.lock().unwrap().push_back(response);
    }

    pub fn queue_tool_call(&self, name: &str, params: serde_json::Value) {
        let response = Response {
            content: ResponseContent::ToolCalls(vec![ToolCall {
                id: format!("call_{}", uuid::Uuid::new_v4()),
                name: name.to_string(),
                parameters: params,
            }]),
            stop_reason: Some("tool_use".to_string()),
        };
        self.responses.lock().unwrap().push_back(response);
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

        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("No more mock responses queued"))
    }
}
