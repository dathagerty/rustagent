use super::{LlmClient, Message, Response, ResponseContent, Role, ToolCall, ToolDefinition};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::time::{Duration, sleep};
use tracing::{info, instrument, warn};

pub struct OllamaClient {
    base_url: String,
    model: String,
    client: Client,
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

    async fn send_request(&self, request_body: &serde_json::Value) -> anyhow::Result<Response> {
        let url = format!("{}/api/chat", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API error {}: {}", status, body);
        }

        let ollama_response: OllamaResponse = response.json().await?;

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

        let mut retries = 0;
        let max_retries = 3;

        loop {
            let error_msg = match self.send_request(&request_body).await {
                Ok(response) => {
                    info!("Ollama API call successful");
                    return Ok(response);
                }
                Err(e) => {
                    let msg = e.to_string();
                    if retries >= max_retries || !is_retryable_error(&msg) {
                        warn!(error = %msg, "Ollama API call failed permanently");
                        return Err(e);
                    }
                    msg
                }
            };

            retries += 1;
            let delay = Duration::from_secs(2u64.pow(retries));
            warn!(
                attempt = retries,
                max_retries = max_retries,
                delay_secs = delay.as_secs(),
                error = %error_msg,
                "Ollama API call failed, retrying"
            );
            sleep(delay).await;
        }
    }
}

fn is_retryable_error(error_msg: &str) -> bool {
    let msg = error_msg.to_lowercase();
    msg.contains("connection refused")
        || msg.contains("timeout")
        || msg.contains("connection reset")
}
