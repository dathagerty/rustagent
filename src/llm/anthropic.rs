use super::{LlmClient, Message, Response, ResponseContent, Role, ToolCall, ToolDefinition};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::time::{Duration, sleep};
use tracing::{info, instrument, warn};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicClient {
    api_key: String,
    model: String,
    max_tokens: u32,
    client: Client,
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

impl AnthropicClient {
    pub fn new(api_key: String, model: String, max_tokens: u32) -> Self {
        Self {
            api_key,
            model,
            max_tokens,
            client: Client::new(),
        }
    }

    pub fn format_request(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<serde_json::Value> {
        // Extract system message
        let system_message = messages
            .iter()
            .find(|m| m.role == Role::System)
            .map(|m| m.content.clone());

        // Filter out system messages and tool messages from messages array
        // (Tool messages need special handling in Anthropic - for now we skip them
        // and rely on the calling code to format tool results as user messages)
        let anthropic_messages: Vec<AnthropicMessage> = messages
            .iter()
            .filter(|m| m.role != Role::System && m.role != Role::Tool)
            .map(|m| AnthropicMessage {
                role: match m.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                    Role::System => unreachable!("System messages filtered out"),
                    Role::Tool => unreachable!("Tool messages filtered out"),
                },
                content: m.content.clone(),
            })
            .collect();

        let anthropic_tools: Option<Vec<AnthropicTool>> = if tools.is_empty() {
            None
        } else {
            Some(
                tools
                    .iter()
                    .map(|t| AnthropicTool {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        input_schema: t.parameters.clone(),
                    })
                    .collect(),
            )
        };

        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            system: system_message,
            messages: anthropic_messages,
            tools: anthropic_tools,
        };

        Ok(serde_json::to_value(request)?)
    }
}

#[async_trait]
impl LlmClient for AnthropicClient {
    #[instrument(skip(self, messages, tools), fields(model = %self.model))]
    async fn chat(
        &self,
        messages: Vec<Message>,
        tools: &[ToolDefinition],
    ) -> anyhow::Result<Response> {
        info!(message_count = messages.len(), tool_count = tools.len(), "Starting Anthropic API call");
        let request_body = self.format_request(&messages, tools)?;

        let mut retries = 0;
        let max_retries = 3;

        loop {
            let error_msg = match self.send_request(&request_body).await {
                Ok(response) => {
                    info!("Anthropic API call successful");
                    return Ok(response);
                }
                Err(e) => {
                    let msg = e.to_string();
                    if retries >= max_retries || !is_retryable_error(&msg) {
                        warn!(error = %msg, "Anthropic API call failed permanently");
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
                "Anthropic API call failed, retrying"
            );
            sleep(delay).await;
        }
    }
}

impl AnthropicClient {
    async fn send_request(
        &self,
        request_body: &serde_json::Value,
    ) -> anyhow::Result<Response> {
        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error {}: {}", status, body);
        }

        let anthropic_response: AnthropicResponse = response.json().await?;

        let content = if anthropic_response
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolUse { .. }))
        {
            let tool_calls: Vec<ToolCall> = anthropic_response
                .content
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::ToolUse { id, name, input } => Some(ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        parameters: input.clone(),
                    }),
                    _ => None,
                })
                .collect();
            ResponseContent::ToolCalls(tool_calls)
        } else {
            let text = anthropic_response
                .content
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            ResponseContent::Text(text)
        };

        Ok(Response {
            content,
            stop_reason: anthropic_response.stop_reason,
        })
    }
}

pub fn is_retryable_error(error_msg: &str) -> bool {
    let msg = error_msg.to_lowercase();
    msg.contains("rate limit")
        || msg.contains("timeout")
        || msg.contains("connection")
        || msg.contains("network")
        || msg.contains("502")
        || msg.contains("503")
        || msg.contains("504")
}
