use super::error::LlmError;
use super::retry::{RetryConfig, parse_retry_after_header, parse_retry_from_message, with_retry};
use super::{LlmClient, Message, Response, ResponseContent, Role, ToolCall, ToolDefinition};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const PROVIDER: &str = "anthropic";

pub struct AnthropicClient {
    api_key: String,
    model: String,
    max_tokens: u32,
    client: Client,
    retry_config: RetryConfig,
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
            retry_config: RetryConfig::default(),
        }
    }

    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    pub fn format_request(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<serde_json::Value> {
        let system_message = messages
            .iter()
            .find(|m| m.role == Role::System)
            .map(|m| m.content.clone());

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

    async fn send_request_once(
        &self,
        request_body: &serde_json::Value,
    ) -> Result<Response, LlmError> {
        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| LlmError::network(PROVIDER, Some(e.to_string())))?;

        if !response.status().is_success() {
            let status = response.status();
            let retry_after = parse_retry_after_header(&response);
            let body = response.text().await.unwrap_or_default();

            let retry_after = retry_after.or_else(|| parse_retry_from_message(&body));

            return Err(LlmError::from_status(PROVIDER, status, body, retry_after));
        }

        let anthropic_response: AnthropicResponse = response
            .json()
            .await
            .map_err(|e| LlmError::bad_request(PROVIDER, Some(e.to_string())))?;

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

#[async_trait]
impl LlmClient for AnthropicClient {
    #[instrument(skip(self, messages, tools), fields(model = %self.model))]
    async fn chat(
        &self,
        messages: Vec<Message>,
        tools: &[ToolDefinition],
    ) -> anyhow::Result<Response> {
        info!(
            message_count = messages.len(),
            tool_count = tools.len(),
            "Starting Anthropic API call"
        );

        let request_body = self.format_request(&messages, tools)?;

        let result = with_retry(PROVIDER, &self.retry_config, || {
            self.send_request_once(&request_body)
        })
        .await;

        match result {
            Ok(response) => {
                info!("Anthropic API call successful");
                Ok(response)
            }
            Err(e) => Err(e.into()),
        }
    }
}
