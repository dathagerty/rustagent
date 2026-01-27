use super::error::LlmError;
use super::retry::{with_retry, RetryConfig};
use super::{LlmClient, Message, Response, ResponseContent, Role, ToolCall, ToolDefinition};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

const PROVIDER: &str = "ollama";

pub struct OllamaClient {
    base_url: String,
    model: String,
    client: Client,
    retry_config: RetryConfig,
}

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OllamaTool>>,
}

#[derive(Debug, Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct OllamaTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OllamaFunction,
}

#[derive(Debug, Serialize)]
struct OllamaFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    message: OllamaResponseMessage,
    #[allow(dead_code)]
    done: bool,
    done_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseMessage {
    #[allow(dead_code)]
    role: String,
    content: String,
    #[serde(default)]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OllamaToolCall {
    function: OllamaFunctionCall,
}

#[derive(Debug, Deserialize)]
struct OllamaFunctionCall {
    name: String,
    arguments: serde_json::Value,
}

impl OllamaClient {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            base_url,
            model,
            client: Client::new(),
            retry_config: RetryConfig::default(),
        }
    }

    pub fn format_request(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<serde_json::Value> {
        let ollama_messages: Vec<OllamaMessage> = messages
            .iter()
            .filter(|m| m.role != Role::Tool)
            .map(|m| OllamaMessage {
                role: match m.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                    Role::System => "system".to_string(),
                    Role::Tool => unreachable!("Tool messages filtered out"),
                },
                content: m.content.clone(),
            })
            .collect();

        let ollama_tools: Option<Vec<OllamaTool>> = if tools.is_empty() {
            None
        } else {
            Some(
                tools
                    .iter()
                    .map(|t| OllamaTool {
                        tool_type: "function".to_string(),
                        function: OllamaFunction {
                            name: t.name.clone(),
                            description: t.description.clone(),
                            parameters: t.parameters.clone(),
                        },
                    })
                    .collect(),
            )
        };

        let request = OllamaRequest {
            model: self.model.clone(),
            messages: ollama_messages,
            stream: false,
            tools: ollama_tools,
        };

        Ok(serde_json::to_value(request)?)
    }

    async fn send_request_once(
        &self,
        request_body: &serde_json::Value,
    ) -> Result<Response, LlmError> {
        let url = format!("{}/api/chat", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| LlmError::network(PROVIDER, Some(e.to_string())))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(LlmError::from_status(PROVIDER, status, body, None));
        }

        let ollama_response: OllamaResponse = response
            .json()
            .await
            .map_err(|e| LlmError::bad_request(PROVIDER, Some(e.to_string())))?;

        let content = if let Some(tool_calls) = ollama_response.message.tool_calls {
            let calls: Vec<ToolCall> = tool_calls
                .into_iter()
                .enumerate()
                .map(|(i, tc)| {
                    let parameters = parse_arguments(&tc.function.arguments);
                    ToolCall {
                        id: format!("call_{}", i),
                        name: tc.function.name,
                        parameters,
                    }
                })
                .collect();
            ResponseContent::ToolCalls(calls)
        } else {
            ResponseContent::Text(ollama_response.message.content)
        };

        Ok(Response {
            content,
            stop_reason: ollama_response.done_reason,
        })
    }
}

fn parse_arguments(args: &serde_json::Value) -> serde_json::Value {
    match args {
        serde_json::Value::String(s) => serde_json::from_str(s).unwrap_or(serde_json::Value::Null),
        other => other.clone(),
    }
}

#[async_trait]
impl LlmClient for OllamaClient {
    #[instrument(skip(self, messages, tools), fields(model = %self.model))]
    async fn chat(
        &self,
        messages: Vec<Message>,
        tools: &[ToolDefinition],
    ) -> anyhow::Result<Response> {
        info!(
            message_count = messages.len(),
            tool_count = tools.len(),
            "Starting Ollama API call"
        );

        let request_body = self.format_request(&messages, tools)?;

        let result = with_retry(PROVIDER, &self.retry_config, || {
            self.send_request_once(&request_body)
        })
        .await;

        match result {
            Ok(response) => {
                info!("Ollama API call successful");
                Ok(response)
            }
            Err(e) => Err(anyhow::anyhow!("{}", e)),
        }
    }
}
