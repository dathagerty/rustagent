use super::error::LlmError;
use super::retry::{RetryConfig, parse_retry_after_header, parse_retry_from_message, with_retry};
use super::{LlmClient, Message, Response, ResponseContent, Role, ToolCall, ToolDefinition};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";
const PROVIDER: &str = "openai";

pub struct OpenAiClient {
    api_key: String,
    model: String,
    max_tokens: u32,
    client: Client,
    retry_config: RetryConfig,
}

#[derive(Debug, Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiTool>>,
}

#[derive(Debug, Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAiFunction,
}

#[derive(Debug, Serialize)]
struct OpenAiFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCall {
    id: String,
    function: OpenAiFunctionCall,
}

#[derive(Debug, Deserialize)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}

impl OpenAiClient {
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
        let openai_messages: Vec<OpenAiMessage> = messages
            .iter()
            .map(|m| OpenAiMessage {
                role: match m.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                    Role::System => "system".to_string(),
                    Role::Tool => "tool".to_string(),
                },
                content: m.content.clone(),
                tool_call_id: m.tool_call_id.clone(),
            })
            .collect();

        let openai_tools: Option<Vec<OpenAiTool>> = if tools.is_empty() {
            None
        } else {
            Some(
                tools
                    .iter()
                    .map(|t| OpenAiTool {
                        tool_type: "function".to_string(),
                        function: OpenAiFunction {
                            name: t.name.clone(),
                            description: t.description.clone(),
                            parameters: t.parameters.clone(),
                        },
                    })
                    .collect(),
            )
        };

        let request = OpenAiRequest {
            model: self.model.clone(),
            messages: openai_messages,
            max_tokens: self.max_tokens,
            tools: openai_tools,
        };

        Ok(serde_json::to_value(request)?)
    }

    async fn send_request_once(
        &self,
        request_body: &serde_json::Value,
    ) -> Result<Response, LlmError> {
        let response = self
            .client
            .post(OPENAI_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
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

        let openai_response: OpenAiResponse = response
            .json()
            .await
            .map_err(|e| LlmError::bad_request(PROVIDER, Some(e.to_string())))?;

        let choice = openai_response.choices.first().ok_or_else(|| {
            LlmError::bad_request(PROVIDER, Some("No choices in response".to_string()))
        })?;

        let content = if let Some(tool_calls) = &choice.message.tool_calls {
            let calls: Vec<ToolCall> = tool_calls
                .iter()
                .map(|tc| ToolCall {
                    id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    parameters: serde_json::from_str(&tc.function.arguments)
                        .unwrap_or(serde_json::Value::Null),
                })
                .collect();
            ResponseContent::ToolCalls(calls)
        } else {
            ResponseContent::Text(choice.message.content.clone().unwrap_or_default())
        };

        Ok(Response {
            content,
            stop_reason: choice.finish_reason.clone(),
        })
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    #[instrument(skip(self, messages, tools), fields(model = %self.model))]
    async fn chat(
        &self,
        messages: Vec<Message>,
        tools: &[ToolDefinition],
    ) -> anyhow::Result<Response> {
        info!(
            message_count = messages.len(),
            tool_count = tools.len(),
            "Starting OpenAI API call"
        );

        let request_body = self.format_request(&messages, tools)?;

        let result = with_retry(PROVIDER, &self.retry_config, || {
            self.send_request_once(&request_body)
        })
        .await;

        match result {
            Ok(response) => {
                info!("OpenAI API call successful");
                Ok(response)
            }
            Err(e) => Err(e.into()),
        }
    }
}
