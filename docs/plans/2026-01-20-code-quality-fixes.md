# Code Quality Fixes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Address all code quality issues identified in the codebase review: eliminate code duplication, implement missing providers, fix error handling, add proper logging, and improve test coverage.

**Architecture:** Extract shared logic into reusable components, implement OpenAI/Ollama clients following the existing Anthropic pattern, replace magic strings with structured tool calls, add tracing with file-based log rotation.

**Tech Stack:** Rust 2024 edition, tracing + tracing-subscriber + tracing-appender for logging, existing async-trait/tokio/reqwest stack.

---

## Phase 1: Foundation Fixes

### Task 1: Extend Message Model for Tool Results

**Files:**
- Modify: `src/llm/mod.rs`
- Create: `tests/message_test.rs`

**Rationale:** OpenAI requires tool results to be sent as `role: "tool"` messages with a `tool_call_id`. The current model only supports User/Assistant/System and appends tool results as User messages, which breaks OpenAI compatibility.

**Step 1: Write tests for new message types**

Create `tests/message_test.rs`:
```rust
use rustagent::llm::{Message, Role};

#[test]
fn test_tool_message_creation() {
	let msg = Message::tool_result("call_123", "File contents here");
	assert_eq!(msg.role, Role::Tool);
	assert_eq!(msg.tool_call_id, Some("call_123".to_string()));
	assert_eq!(msg.content, "File contents here");
}

#[test]
fn test_user_message_has_no_tool_call_id() {
	let msg = Message::user("Hello");
	assert_eq!(msg.role, Role::User);
	assert!(msg.tool_call_id.is_none());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_tool_message`
Expected: FAIL - no `tool_result` method

**Step 3: Extend Message struct and Role enum**

In `src/llm/mod.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
	User,
	Assistant,
	System,
	Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
	pub role: Role,
	pub content: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub tool_call_id: Option<String>,
}

impl Message {
	pub fn user(content: impl Into<String>) -> Self {
		Self {
			role: Role::User,
			content: content.into(),
			tool_call_id: None,
		}
	}

	pub fn assistant(content: impl Into<String>) -> Self {
		Self {
			role: Role::Assistant,
			content: content.into(),
			tool_call_id: None,
		}
	}

	pub fn system(content: impl Into<String>) -> Self {
		Self {
			role: Role::System,
			content: content.into(),
			tool_call_id: None,
		}
	}

	pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
		Self {
			role: Role::Tool,
			content: content.into(),
			tool_call_id: Some(tool_call_id.into()),
		}
	}
}
```

**Step 4: Update AnthropicClient to handle tool results**

In `src/llm/anthropic.rs`, update message formatting to handle Role::Tool (Anthropic uses a different format - tool results go in the user turn with tool_result blocks, but for now we can skip Tool messages in the Anthropic format since we'll refactor the loop).

**Step 5: Update ralph and planning loops to use Message constructors**

Replace manual Message struct creation with the new constructors. For tool results:
```rust
// Before (in ralph/mod.rs execute_task):
messages.push(Message {
	role: Role::User,
	content: format!("Tool result for {}:\n{}", tool_call.name, output),
});

// After:
messages.push(Message::tool_result(&tool_call.id, &output));
```

**Step 6: Run tests**

Run: `cargo test`
Expected: All tests pass

**Step 7: Commit**

```bash
git add src/llm/mod.rs tests/message_test.rs src/ralph/mod.rs src/planning/mod.rs src/llm/anthropic.rs
git commit -m "feat: extend Message model with Role::Tool and tool_call_id for provider compatibility"
```

---

### Task 2: Unify Error Handling in LlmClient Trait

**Files:**
- Modify: `src/llm/mod.rs`
- Modify: `src/llm/anthropic.rs`
- Modify: `src/planning/mod.rs`
- Modify: `src/ralph/mod.rs`

**Step 1: Update LlmClient trait to use anyhow::Result**

In `src/llm/mod.rs`, change:
```rust
async fn chat(
	&self,
	messages: Vec<Message>,
	tools: &[ToolDefinition],
) -> Result<Response, Box<dyn std::error::Error>>;
```

To:
```rust
async fn chat(
	&self,
	messages: Vec<Message>,
	tools: &[ToolDefinition],
) -> anyhow::Result<Response>;
```

**Step 2: Update AnthropicClient implementation**

In `src/llm/anthropic.rs`:
- Change `format_request` return type from `Result<..., Box<dyn std::error::Error>>` to `anyhow::Result<...>`
- Change `chat` return type similarly
- Change `send_request` return type similarly
- Remove all `Box::new(e) as Box<dyn std::error::Error>` conversions, use `?` directly

**Step 3: Update callers in planning and ralph modules**

Remove `.map_err(|e| anyhow::anyhow!(...))` wrappers since errors now propagate naturally.

**Step 4: Run tests**

Run: `cargo test`
Expected: All tests pass

**Step 5: Commit**

```bash
git add src/llm/mod.rs src/llm/anthropic.rs src/planning/mod.rs src/ralph/mod.rs
git commit -m "refactor: unify LlmClient error handling to use anyhow::Result"
```

---

### Task 3: Extract LLM Client Factory

**Files:**
- Create: `src/llm/factory.rs`
- Modify: `src/llm/mod.rs`
- Modify: `src/planning/mod.rs`
- Modify: `src/ralph/mod.rs`

**Step 1: Create factory module**

Create `src/llm/factory.rs`:
```rust
use crate::config::{Config, LlmConfig, LlmProvider};
use crate::llm::anthropic::AnthropicClient;
use crate::llm::LlmClient;
use anyhow::{bail, Result};
use std::sync::Arc;

pub fn create_client(config: &Config, llm_config: &LlmConfig) -> Result<Arc<dyn LlmClient>> {
	match llm_config.provider {
		LlmProvider::Anthropic => {
			let anthropic_config = config
				.anthropic
				.as_ref()
				.ok_or_else(|| anyhow::anyhow!("Anthropic provider selected but [anthropic] config missing"))?;

			Ok(Arc::new(AnthropicClient::new(
				anthropic_config.api_key.clone(),
				llm_config.model.clone(),
				llm_config.max_tokens,
			)))
		}
		LlmProvider::OpenAi => {
			bail!("OpenAI provider not yet implemented")
		}
		LlmProvider::Ollama => {
			bail!("Ollama provider not yet implemented")
		}
	}
}
```

**Step 2: Export from llm/mod.rs**

Add to `src/llm/mod.rs`:
```rust
pub mod factory;
```

**Step 3: Update PlanningAgent to use factory**

In `src/planning/mod.rs`, replace the client creation match block with:
```rust
use crate::llm::factory::create_client;

// In new():
let llm_config = config.planning_llm().clone();
let client = create_client(&config, &llm_config)?;
```

Change `client` field type from `Box<dyn LlmClient>` to `Arc<dyn LlmClient>`.

**Step 4: Update RalphLoop similarly**

In `src/ralph/mod.rs`, use the same factory pattern.

**Step 5: Run tests**

Run: `cargo test`
Expected: All tests pass

**Step 6: Commit**

```bash
git add src/llm/factory.rs src/llm/mod.rs src/planning/mod.rs src/ralph/mod.rs
git commit -m "refactor: extract LLM client factory to eliminate duplication"
```

---

### Task 4: Extract Tool Registry Factory

**Files:**
- Create: `src/tools/factory.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/planning/mod.rs`
- Modify: `src/ralph/mod.rs`

**Step 1: Create tools factory**

Create `src/tools/factory.rs`:
```rust
use crate::config::Config;
use crate::security::permission::PermissionHandler;
use crate::security::SecurityValidator;
use crate::tools::file::{ListFilesTool, ReadFileTool, WriteFileTool};
use crate::tools::shell::RunCommandTool;
use crate::tools::ToolRegistry;
use std::sync::Arc;

pub fn create_default_registry(
	validator: Arc<SecurityValidator>,
	permission_handler: Arc<dyn PermissionHandler>,
) -> ToolRegistry {
	let registry = ToolRegistry::new();

	registry.register(Arc::new(ReadFileTool::new(
		validator.clone(),
		permission_handler.clone(),
	)));
	registry.register(Arc::new(WriteFileTool::new(
		validator.clone(),
		permission_handler.clone(),
	)));
	registry.register(Arc::new(ListFilesTool::new(
		validator.clone(),
		permission_handler.clone(),
	)));
	registry.register(Arc::new(RunCommandTool::new(
		validator.clone(),
		permission_handler.clone(),
	)));

	registry
}
```

**Step 2: Export from tools/mod.rs**

Add to `src/tools/mod.rs`:
```rust
pub mod factory;
```

**Step 3: Update PlanningAgent and RalphLoop**

Replace the 4-line register blocks with:
```rust
use crate::tools::factory::create_default_registry;

let registry = create_default_registry(validator, permission_handler);
```

**Step 4: Run tests**

Run: `cargo test`
Expected: All tests pass

**Step 5: Commit**

```bash
git add src/tools/factory.rs src/tools/mod.rs src/planning/mod.rs src/ralph/mod.rs
git commit -m "refactor: extract tool registry factory to eliminate duplication"
```

---

### Task 5: Extract Permission Checking Trait

**Files:**
- Create: `src/tools/permission_check.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/tools/file.rs`

**Step 1: Create permission check helper**

Create `src/tools/permission_check.rs`:
```rust
use crate::security::permission::{
	PermissionHandler, PermissionRequest, PermissionResult, ResourceType,
};
use crate::security::{SecurityValidator, ValidationResult};
use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, RwLock};

pub struct FilePermissionChecker {
	validator: Arc<SecurityValidator>,
	permission_handler: Arc<dyn PermissionHandler>,
	runtime_allowed: Arc<RwLock<HashSet<String>>>,
}

impl FilePermissionChecker {
	pub fn new(
		validator: Arc<SecurityValidator>,
		permission_handler: Arc<dyn PermissionHandler>,
	) -> Self {
		Self {
			validator,
			permission_handler,
			runtime_allowed: Arc::new(RwLock::new(HashSet::new())),
		}
	}

	pub fn check_permission(&self, path: &Path) -> Result<()> {
		let path_str = path.to_string_lossy().to_string();

		// Check runtime allowed
		{
			let allowed = self.runtime_allowed.read()
				.map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
			if allowed.contains(&path_str) {
				return Ok(());
			}
		}

		// Validate path
		match self.validator.validate_file_path(path) {
			ValidationResult::Allowed => Ok(()),
			ValidationResult::Denied(reason) => {
				anyhow::bail!("Path denied: {}", reason)
			}
			ValidationResult::RequiresPermission(reason) => {
				let request = PermissionRequest {
					resource_type: ResourceType::FilePath,
					action: path_str.clone(),
					reason,
				};

				match self.permission_handler.request_permission(&request) {
					PermissionResult::Allow => Ok(()),
					PermissionResult::Deny => {
						anyhow::bail!("Permission denied by user")
					}
					PermissionResult::AllowAlways(p) => {
						let mut allowed = self.runtime_allowed.write()
							.map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
						allowed.insert(p);
						Ok(())
					}
				}
			}
		}
	}
}
```

**Step 2: Export from tools/mod.rs**

Add: `pub mod permission_check;`

**Step 3: Refactor file tools to use FilePermissionChecker**

In `src/tools/file.rs`, replace the duplicated `check_permission` methods and `runtime_allowed` fields with a single `FilePermissionChecker` field in each tool.

**Step 4: Run tests**

Run: `cargo test`
Expected: All tests pass

**Step 5: Commit**

```bash
git add src/tools/permission_check.rs src/tools/mod.rs src/tools/file.rs
git commit -m "refactor: extract file permission checking to eliminate 3x duplication"
```

---

## Phase 2: Logging (Early for Debugging)

### Task 6: Add File-Based Logging with Tracing

**Files:**
- Modify: `Cargo.toml`
- Create: `src/logging.rs`
- Modify: `src/lib.rs`
- Modify: `src/main.rs`

**Rationale:** Adding logging early helps debug provider implementations. Moving this before provider work per oracle recommendation.

**Step 1: Add dependencies**

Add to `Cargo.toml`:
```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-appender = "0.2"
```

Remove `log` and `env_logger` dependencies (or keep for compatibility).

**Step 2: Create logging module**

Create `src/logging.rs`:
```rust
use anyhow::Result;
use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
	fmt,
	layer::SubscriberExt,
	util::SubscriberInitExt,
	EnvFilter,
};

pub fn init_logging() -> Result<WorkerGuard> {
	let log_dir = get_log_directory()?;
	std::fs::create_dir_all(&log_dir)?;

	let file_appender = RollingFileAppender::new(
		Rotation::DAILY,
		&log_dir,
		"rustagent.log",
	);

	let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

	let file_layer = fmt::layer()
		.with_writer(non_blocking)
		.with_ansi(false)
		.with_target(true)
		.with_thread_ids(true);

	let filter = EnvFilter::try_from_default_env()
		.unwrap_or_else(|_| EnvFilter::new("rustagent=info"));

	tracing_subscriber::registry()
		.with(filter)
		.with(file_layer)
		.init();

	Ok(guard)
}

fn get_log_directory() -> Result<PathBuf> {
	if let Ok(state_home) = std::env::var("XDG_STATE_HOME") {
		return Ok(PathBuf::from(state_home).join("rustagent").join("logs"));
	}

	if let Some(data_dir) = dirs::data_local_dir() {
		return Ok(data_dir.join("rustagent").join("logs"));
	}

	if let Some(home) = dirs::home_dir() {
		return Ok(home.join(".local").join("state").join("rustagent").join("logs"));
	}

	anyhow::bail!("Could not determine log directory")
}

pub fn get_log_directory_path() -> Option<PathBuf> {
	get_log_directory().ok()
}
```

**Step 3: Export from lib.rs**

Add to `src/lib.rs`:
```rust
pub mod logging;
```

**Step 4: Initialize in main.rs**

In `src/main.rs`:
```rust
use rustagent::logging;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	// Hold the guard for program lifetime
	let _log_guard = logging::init_logging()?;

	// ... rest of main
}
```

**Step 5: Add tracing instrumentation to LLM clients**

In `src/llm/anthropic.rs`, add spans:
```rust
use tracing::{info, warn, instrument};

#[instrument(skip(self, messages, tools), fields(model = %self.model))]
async fn chat(...) { ... }
```

**Step 6: Run tests**

Run: `cargo test`
Expected: All tests pass

**Step 7: Commit**

```bash
git add Cargo.toml src/logging.rs src/lib.rs src/main.rs src/llm/anthropic.rs
git commit -m "feat: add file-based logging with tracing and XDG directory support"
```

---

## Phase 3: Implement Missing Providers

### Task 7: Implement OpenAI Client

**Files:**
- Create: `src/llm/openai.rs`
- Create: `tests/openai_test.rs`
- Modify: `src/llm/mod.rs`
- Modify: `src/llm/factory.rs`

**Step 1: Write tests for OpenAI client**

Create `tests/openai_test.rs`:
```rust
use rustagent::llm::openai::OpenAiClient;
use rustagent::llm::{Message, Role, ToolDefinition};

#[test]
fn test_openai_format_request_basic() {
	let client = OpenAiClient::new(
		"test-key".to_string(),
		"gpt-4".to_string(),
		4096,
	);

	let messages = vec![
		Message {
			role: Role::User,
			content: "Hello".to_string(),
		},
	];

	let request = client.format_request(&messages, &[]).unwrap();

	assert_eq!(request["model"], "gpt-4");
	assert_eq!(request["messages"][0]["role"], "user");
	assert_eq!(request["messages"][0]["content"], "Hello");
}

#[test]
fn test_openai_format_request_with_system() {
	let client = OpenAiClient::new(
		"test-key".to_string(),
		"gpt-4".to_string(),
		4096,
	);

	let messages = vec![
		Message {
			role: Role::System,
			content: "You are helpful".to_string(),
		},
		Message {
			role: Role::User,
			content: "Hello".to_string(),
		},
	];

	let request = client.format_request(&messages, &[]).unwrap();

	// OpenAI keeps system in messages array
	assert_eq!(request["messages"][0]["role"], "system");
	assert_eq!(request["messages"][1]["role"], "user");
}

#[test]
fn test_openai_format_request_with_tools() {
	let client = OpenAiClient::new(
		"test-key".to_string(),
		"gpt-4".to_string(),
		4096,
	);

	let messages = vec![Message {
		role: Role::User,
		content: "Read file.txt".to_string(),
	}];

	let tools = vec![ToolDefinition {
		name: "read_file".to_string(),
		description: "Read a file".to_string(),
		parameters: serde_json::json!({
			"type": "object",
			"properties": {
				"path": {"type": "string"}
			},
			"required": ["path"]
		}),
	}];

	let request = client.format_request(&messages, &tools).unwrap();

	assert!(request["tools"].is_array());
	assert_eq!(request["tools"][0]["type"], "function");
	assert_eq!(request["tools"][0]["function"]["name"], "read_file");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_openai`
Expected: FAIL with "no `openai` module"

**Step 3: Implement OpenAI client**

Create `src/llm/openai.rs`:
```rust
use super::{LlmClient, Message, Response, ResponseContent, Role, ToolCall, ToolDefinition};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::time::{Duration, sleep};

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";

pub struct OpenAiClient {
	api_key: String,
	model: String,
	max_tokens: u32,
	client: Client,
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
		}
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
				},
				content: m.content.clone(),
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

	async fn send_request(
		&self,
		request_body: &serde_json::Value,
	) -> anyhow::Result<Response> {
		let response = self
			.client
			.post(OPENAI_API_URL)
			.header("Authorization", format!("Bearer {}", self.api_key))
			.header("Content-Type", "application/json")
			.json(&request_body)
			.send()
			.await?;

		if !response.status().is_success() {
			let status = response.status();
			let body = response.text().await.unwrap_or_default();
			anyhow::bail!("OpenAI API error {}: {}", status, body);
		}

		let openai_response: OpenAiResponse = response.json().await?;

		let choice = openai_response
			.choices
			.first()
			.ok_or_else(|| anyhow::anyhow!("No choices in response"))?;

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
	async fn chat(
		&self,
		messages: Vec<Message>,
		tools: &[ToolDefinition],
	) -> anyhow::Result<Response> {
		let request_body = self.format_request(&messages, tools)?;

		let mut retries = 0;
		let max_retries = 3;

		loop {
			let error_msg = match self.send_request(&request_body).await {
				Ok(response) => return Ok(response),
				Err(e) => {
					let msg = e.to_string();
					if retries >= max_retries || !is_retryable_error(&msg) {
						return Err(e);
					}
					msg
				}
			};

			retries += 1;
			let delay = Duration::from_secs(2u64.pow(retries));
			eprintln!(
				"OpenAI API call failed, retrying in {:?} (attempt {}/{}): {}",
				delay, retries, max_retries, error_msg
			);
			sleep(delay).await;
		}
	}
}

fn is_retryable_error(error_msg: &str) -> bool {
	let msg = error_msg.to_lowercase();
	msg.contains("rate limit")
		|| msg.contains("timeout")
		|| msg.contains("connection")
		|| msg.contains("429")
		|| msg.contains("502")
		|| msg.contains("503")
		|| msg.contains("504")
}
```

**Step 4: Export and wire up factory**

In `src/llm/mod.rs`, add:
```rust
pub mod openai;
```

In `src/llm/factory.rs`, add the OpenAI branch:
```rust
use crate::llm::openai::OpenAiClient;

LlmProvider::OpenAi => {
	let openai_config = config
		.openai
		.as_ref()
		.ok_or_else(|| anyhow::anyhow!("OpenAI provider selected but [openai] config missing"))?;

	Ok(Arc::new(OpenAiClient::new(
		openai_config.api_key.clone(),
		llm_config.model.clone(),
		llm_config.max_tokens,
	)))
}
```

**Step 5: Run tests**

Run: `cargo test test_openai`
Expected: All tests pass

**Step 6: Commit**

```bash
git add src/llm/openai.rs src/llm/mod.rs src/llm/factory.rs tests/openai_test.rs
git commit -m "feat: implement OpenAI client with tool calling support"
```

---

### Task 8: Implement Ollama Client

**Files:**
- Create: `src/llm/ollama.rs`
- Create: `tests/ollama_test.rs`
- Modify: `src/llm/mod.rs`
- Modify: `src/llm/factory.rs`

**Step 1: Write tests for Ollama client**

Create `tests/ollama_test.rs`:
```rust
use rustagent::llm::ollama::OllamaClient;
use rustagent::llm::{Message, Role, ToolDefinition};

#[test]
fn test_ollama_format_request_basic() {
	let client = OllamaClient::new(
		"http://localhost:11434".to_string(),
		"llama3".to_string(),
	);

	let messages = vec![Message {
		role: Role::User,
		content: "Hello".to_string(),
	}];

	let request = client.format_request(&messages, &[]).unwrap();

	assert_eq!(request["model"], "llama3");
	assert_eq!(request["messages"][0]["role"], "user");
	assert_eq!(request["stream"], false);
}

#[test]
fn test_ollama_format_request_with_tools() {
	let client = OllamaClient::new(
		"http://localhost:11434".to_string(),
		"llama3".to_string(),
	);

	let messages = vec![Message {
		role: Role::User,
		content: "Read file".to_string(),
	}];

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
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_ollama`
Expected: FAIL with "no `ollama` module"

**Step 3: Implement Ollama client**

Create `src/llm/ollama.rs`:
```rust
use super::{LlmClient, Message, Response, ResponseContent, Role, ToolCall, ToolDefinition};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::time::{Duration, sleep};

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
	done: bool,
	done_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseMessage {
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
			.map(|m| OllamaMessage {
				role: match m.role {
					Role::User => "user".to_string(),
					Role::Assistant => "assistant".to_string(),
					Role::System => "system".to_string(),
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

	async fn send_request(
		&self,
		request_body: &serde_json::Value,
	) -> anyhow::Result<Response> {
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
				.map(|(i, tc)| ToolCall {
					id: format!("call_{}", i),
					name: tc.function.name,
					parameters: tc.function.arguments,
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

#[async_trait]
impl LlmClient for OllamaClient {
	async fn chat(
		&self,
		messages: Vec<Message>,
		tools: &[ToolDefinition],
	) -> anyhow::Result<Response> {
		let request_body = self.format_request(&messages, tools)?;

		let mut retries = 0;
		let max_retries = 3;

		loop {
			let error_msg = match self.send_request(&request_body).await {
				Ok(response) => return Ok(response),
				Err(e) => {
					let msg = e.to_string();
					if retries >= max_retries || !is_retryable_error(&msg) {
						return Err(e);
					}
					msg
				}
			};

			retries += 1;
			let delay = Duration::from_secs(2u64.pow(retries));
			eprintln!(
				"Ollama API call failed, retrying in {:?} (attempt {}/{}): {}",
				delay, retries, max_retries, error_msg
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
```

**Step 4: Export and wire up factory**

In `src/llm/mod.rs`, add:
```rust
pub mod ollama;
```

In `src/llm/factory.rs`, add the Ollama branch:
```rust
use crate::llm::ollama::OllamaClient;

LlmProvider::Ollama => {
	let ollama_config = config
		.ollama
		.as_ref()
		.ok_or_else(|| anyhow::anyhow!("Ollama provider selected but [ollama] config missing"))?;

	Ok(Arc::new(OllamaClient::new(
		ollama_config.base_url.clone(),
		llm_config.model.clone(),
	)))
}
```

**Step 5: Run tests**

Run: `cargo test test_ollama`
Expected: All tests pass

**Step 6: Commit**

```bash
git add src/llm/ollama.rs src/llm/mod.rs src/llm/factory.rs tests/ollama_test.rs
git commit -m "feat: implement Ollama client with tool calling support"
```

---

## Phase 4: Implement Init Command and Signal Tool

### Task 9: Implement Init Command

**Files:**
- Modify: `src/main.rs`
- Create: `tests/init_test.rs`

**Step 1: Write test for init command**

Create `tests/init_test.rs`:
```rust
use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_init_creates_spec_directory() {
	let temp = TempDir::new().unwrap();
	let spec_dir = temp.path().join("specs");

	// Use the compiled binary directly (no cargo shell-out)
	let exe = env!("CARGO_BIN_EXE_rustagent");

	let output = Command::new(exe)
		.args(["init", "--spec-dir", spec_dir.to_str().unwrap()])
		.current_dir(temp.path())
		.output()
		.unwrap();

	assert!(output.status.success(), "Command failed: {:?}", output);
	assert!(spec_dir.exists());
	assert!(spec_dir.is_dir());
}

#[test]
fn test_init_creates_config_template() {
	let temp = TempDir::new().unwrap();
	let spec_dir = temp.path().join("specs");

	let exe = env!("CARGO_BIN_EXE_rustagent");

	let output = Command::new(exe)
		.args(["init", "--spec-dir", spec_dir.to_str().unwrap()])
		.current_dir(temp.path())
		.output()
		.unwrap();

	assert!(output.status.success(), "Command failed: {:?}", output);

	// Config should be created if it doesn't exist
	let config_path = temp.path().join("rustagent.toml");
	assert!(config_path.exists(), "Config file was not created");
}

#[test]
fn test_init_idempotent() {
	let temp = TempDir::new().unwrap();
	let spec_dir = temp.path().join("specs");

	let exe = env!("CARGO_BIN_EXE_rustagent");

	// Run init twice
	Command::new(exe)
		.args(["init", "--spec-dir", spec_dir.to_str().unwrap()])
		.current_dir(temp.path())
		.output()
		.unwrap();

	let output = Command::new(exe)
		.args(["init", "--spec-dir", spec_dir.to_str().unwrap()])
		.current_dir(temp.path())
		.output()
		.unwrap();

	// Should succeed on second run too
	assert!(output.status.success(), "Second init failed: {:?}", output);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_init`
Expected: Output shows TODO comment behavior

**Step 3: Implement init command**

In `src/main.rs`, replace the init handler:
```rust
Commands::Init { spec_dir } => {
	let dir = spec_dir.clone().unwrap_or_else(|| "specs".to_string());

	// Create spec directory
	let spec_path = std::path::Path::new(&dir);
	if !spec_path.exists() {
		std::fs::create_dir_all(spec_path)?;
		println!("Created spec directory: {}", dir);
	} else {
		println!("Spec directory already exists: {}", dir);
	}

	// Create config template if it doesn't exist
	let config_path = std::path::Path::new("rustagent.toml");
	if !config_path.exists() {
		let template = include_str!("../rustagent.toml.example");
		std::fs::write(config_path, template)?;
		println!("Created config template: rustagent.toml");
		println!("Please edit rustagent.toml to add your API keys.");
	} else {
		println!("Config file already exists: rustagent.toml");
	}

	println!("\nInitialization complete!");
	println!("Next steps:");
	println!("  1. Edit rustagent.toml with your API keys");
	println!("  2. Run 'rustagent plan' to create a spec");
	println!("  3. Run 'rustagent run <spec-file>' to execute");
}
```

**Step 4: Run tests**

Run: `cargo test test_init`
Expected: Tests pass

**Step 5: Commit**

```bash
git add src/main.rs tests/init_test.rs
git commit -m "feat: implement init command to create spec directory and config template"
```

---

### Task 10: Implement Signal Completion Tool

**Files:**
- Create: `src/tools/signal.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/tools/factory.rs`
- Modify: `src/ralph/mod.rs`
- Create: `tests/signal_test.rs`

**Step 1: Write tests for signal tool**

Create `tests/signal_test.rs`:
```rust
use rustagent::tools::signal::{SignalTool, CompletionSignal};

#[tokio::test]
async fn test_signal_complete() {
	let tool = SignalTool::new();

	let params = serde_json::json!({
		"signal": "complete",
		"message": "Task finished successfully"
	});

	let result = tool.execute(params).await.unwrap();
	assert!(result.contains("complete"));
}

#[tokio::test]
async fn test_signal_blocked() {
	let tool = SignalTool::new();

	let params = serde_json::json!({
		"signal": "blocked",
		"reason": "Missing dependency"
	});

	let result = tool.execute(params).await.unwrap();
	assert!(result.contains("blocked"));
}

#[test]
fn test_signal_tool_parameters() {
	let tool = SignalTool::new();
	let params = tool.parameters();

	assert!(params["properties"]["signal"].is_object());
	assert!(params["properties"]["message"].is_object());
	assert!(params["properties"]["reason"].is_object());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_signal`
Expected: FAIL with "no `signal` module"

**Step 3: Implement signal tool**

Create `src/tools/signal.rs`:
```rust
use crate::tools::Tool;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompletionSignal {
	Complete,
	Blocked,
}

#[derive(Debug, Deserialize)]
struct SignalParams {
	signal: CompletionSignal,
	#[serde(default)]
	message: Option<String>,
	#[serde(default)]
	reason: Option<String>,
}

pub struct SignalTool;

impl SignalTool {
	pub fn new() -> Self {
		Self
	}
}

impl Default for SignalTool {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Tool for SignalTool {
	fn name(&self) -> &str {
		"signal_completion"
	}

	fn description(&self) -> &str {
		"Signal task completion or blocked status. Use 'complete' when the task is finished successfully, or 'blocked' if you cannot proceed."
	}

	fn parameters(&self) -> serde_json::Value {
		serde_json::json!({
			"type": "object",
			"properties": {
				"signal": {
					"type": "string",
					"enum": ["complete", "blocked"],
					"description": "The completion signal: 'complete' for success, 'blocked' if unable to proceed"
				},
				"message": {
					"type": "string",
					"description": "Optional message describing what was accomplished (for 'complete')"
				},
				"reason": {
					"type": "string",
					"description": "Required reason explaining why the task is blocked (for 'blocked')"
				}
			},
			"required": ["signal"]
		})
	}

	async fn execute(&self, params: serde_json::Value) -> Result<String> {
		let params: SignalParams = serde_json::from_value(params)?;

		match params.signal {
			CompletionSignal::Complete => {
				let msg = params.message.unwrap_or_else(|| "Task completed".to_string());
				Ok(format!("SIGNAL:complete:{}", msg))
			}
			CompletionSignal::Blocked => {
				let reason = params.reason.unwrap_or_else(|| "Unknown reason".to_string());
				Ok(format!("SIGNAL:blocked:{}", reason))
			}
		}
	}
}
```

**Step 4: Export and add to factory**

In `src/tools/mod.rs`, add:
```rust
pub mod signal;
```

In `src/tools/factory.rs`, add:
```rust
use crate::tools::signal::SignalTool;

// In create_default_registry, add:
registry.register(Arc::new(SignalTool::new()));
```

**Step 5: Update RalphLoop to use signal tool**

In `src/ralph/mod.rs`, replace the text-based signal detection:

```rust
// In execute_task, replace the signal detection logic:
ResponseContent::ToolCalls(tool_calls) => {
	// Check for signal_completion tool call
	for tool_call in &tool_calls {
		if tool_call.name == "signal_completion" {
			let tool = self.tools.get(&tool_call.name).context("Tool not found")?;
			let result = tool.execute(tool_call.parameters.clone()).await?;

			if result.starts_with("SIGNAL:complete:") {
				return Ok("TASK_COMPLETE".to_string());
			} else if result.starts_with("SIGNAL:blocked:") {
				let reason = result.strip_prefix("SIGNAL:blocked:").unwrap_or("");
				// Could store reason in spec here
				return Ok("TASK_BLOCKED".to_string());
			}
		}
	}

	// Execute other tool calls as before...
}
```

Also update the instructions in `build_context`:
```rust
context.push_str("5. When complete, call the signal_completion tool with signal='complete'\n");
context.push_str("6. If blocked, call signal_completion with signal='blocked' and explain why\n");
```

**Step 6: Run tests**

Run: `cargo test`
Expected: All tests pass

**Step 7: Commit**

```bash
git add src/tools/signal.rs src/tools/mod.rs src/tools/factory.rs src/ralph/mod.rs tests/signal_test.rs
git commit -m "feat: add signal_completion tool for structured task completion"
```

---

## Phase 5: Polish and Testing

### Task 11: Fix CliPermissionHandler Exit Behavior

**Files:**
- Modify: `src/security/permission.rs`
- Create: `tests/permission_quit_test.rs`

**Step 1: Write test**

Create `tests/permission_quit_test.rs`:
```rust
use rustagent::security::permission::{PermissionHandler, PermissionRequest, PermissionResult, ResourceType};

// Note: We can't easily test the CLI handler without mocking stdin,
// but we can test that AutoApproveHandler still works
#[test]
fn test_auto_approve_still_works() {
	use rustagent::security::permission::AutoApproveHandler;

	let handler = AutoApproveHandler;
	let request = PermissionRequest {
		resource_type: ResourceType::ShellCommand,
		action: "ls".to_string(),
		reason: "test".to_string(),
	};

	match handler.request_permission(&request) {
		PermissionResult::Allow => (),
		_ => panic!("Expected Allow"),
	}
}
```

**Step 2: Fix CliPermissionHandler**

In `src/security/permission.rs`, change the quit behavior:
```rust
pub enum PermissionResult {
	Allow,
	Deny,
	AllowAlways(String),
	Quit, // Add this variant
}

impl PermissionHandler for CliPermissionHandler {
	fn request_permission(&self, request: &PermissionRequest) -> PermissionResult {
		println!("\n⚠️  Permission Required");
		println!("Type: {:?}", request.resource_type);
		println!("Action: {}", request.action);
		println!("Reason: {}", request.reason);
		println!();
		print!("Allow? [y]es / [n]o / [a]lways allow / [q]uit: ");
		io::stdout().flush().unwrap();

		let mut input = String::new();
		io::stdin().read_line(&mut input).unwrap();

		match input.trim().to_lowercase().as_str() {
			"y" | "yes" => PermissionResult::Allow,
			"a" | "always" => {
				let resource = match request.resource_type {
					ResourceType::ShellCommand => {
						request.action.split_whitespace().next().unwrap_or("")
					}
					ResourceType::FilePath => &request.action,
				};
				PermissionResult::AllowAlways(resource.to_string())
			}
			"q" | "quit" => PermissionResult::Quit,
			_ => PermissionResult::Deny,
		}
	}
}
```

**Step 3: Handle Quit in callers**

In `src/tools/shell.rs` and `src/tools/file.rs` (or `permission_check.rs`), handle the Quit variant:
```rust
PermissionResult::Quit => {
	anyhow::bail!("User requested quit")
}
```

**Step 4: Run tests**

Run: `cargo test`
Expected: All tests pass

**Step 5: Commit**

```bash
git add src/security/permission.rs src/tools/shell.rs src/tools/file.rs tests/permission_quit_test.rs
git commit -m "fix: replace process::exit with proper error propagation in permission handler"
```

---

### Task 12: Add Mock LLM Client for Testing

**Files:**
- Create: `src/llm/mock.rs`
- Modify: `src/llm/mod.rs`
- Create: `tests/mock_llm_test.rs`

**Step 1: Create mock client**

Create `src/llm/mock.rs`:
```rust
use super::{LlmClient, Message, Response, ResponseContent, ToolCall, ToolDefinition};
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A mock LLM client for testing that returns pre-configured responses
pub struct MockLlmClient {
	responses: Arc<Mutex<VecDeque<Response>>>,
	recorded_calls: Arc<Mutex<Vec<(Vec<Message>, Vec<ToolDefinition>)>>>,
}

impl MockLlmClient {
	pub fn new() -> Self {
		Self {
			responses: Arc::new(Mutex::new(VecDeque::new())),
			recorded_calls: Arc::new(Mutex::new(Vec::new())),
		}
	}

	/// Queue a text response
	pub fn queue_text_response(&self, text: &str) {
		let response = Response {
			content: ResponseContent::Text(text.to_string()),
			stop_reason: Some("end_turn".to_string()),
		};
		self.responses.lock().unwrap().push_back(response);
	}

	/// Queue a tool call response
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

	/// Get all recorded calls for verification
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
		// Record the call
		self.recorded_calls
			.lock()
			.unwrap()
			.push((messages, tools.to_vec()));

		// Return next queued response
		self.responses
			.lock()
			.unwrap()
			.pop_front()
			.ok_or_else(|| anyhow::anyhow!("No more mock responses queued"))
	}
}
```

**Step 2: Add uuid dependency**

Add to `Cargo.toml`:
```toml
uuid = { version = "1.0", features = ["v4"] }
```

**Step 3: Export from mod.rs**

Add to `src/llm/mod.rs`:
```rust
pub mod mock;
```

**Step 4: Write test using mock client**

Create `tests/mock_llm_test.rs`:
```rust
use rustagent::llm::mock::MockLlmClient;
use rustagent::llm::{LlmClient, Message, Role, ResponseContent};

#[tokio::test]
async fn test_mock_text_response() {
	let client = MockLlmClient::new();
	client.queue_text_response("Hello, world!");

	let messages = vec![Message {
		role: Role::User,
		content: "Hi".to_string(),
	}];

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

	let messages = vec![Message {
		role: Role::User,
		content: "Read the file".to_string(),
	}];

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

	let msg1 = vec![Message { role: Role::User, content: "First".to_string() }];
	let msg2 = vec![Message { role: Role::User, content: "Second".to_string() }];

	client.chat(msg1, &[]).await.unwrap();
	client.chat(msg2, &[]).await.unwrap();

	let calls = client.get_recorded_calls();
	assert_eq!(calls.len(), 2);
	assert_eq!(calls[0].0[0].content, "First");
	assert_eq!(calls[1].0[0].content, "Second");
}
```

**Step 5: Run tests**

Run: `cargo test test_mock`
Expected: All tests pass

**Step 6: Commit**

```bash
git add Cargo.toml src/llm/mock.rs src/llm/mod.rs tests/mock_llm_test.rs
git commit -m "feat: add MockLlmClient for testing agent behavior"
```

---

### Task 13: Add Unit Tests for Ralph Loop Logic

**Files:**
- Modify: `tests/ralph_test.rs`

**Step 1: Add tests for context building and state transitions**

Add to `tests/ralph_test.rs`:
```rust
use chrono::Utc;
use rustagent::spec::{Spec, Task, TaskStatus};

#[test]
fn test_find_next_task_skips_complete() {
	let spec = Spec {
		name: "test".to_string(),
		description: "Test".to_string(),
		branch_name: "feature/test".to_string(),
		created_at: Utc::now(),
		tasks: vec![
			Task {
				id: "task-1".to_string(),
				title: "Task 1".to_string(),
				description: "First task".to_string(),
				acceptance_criteria: vec![],
				status: TaskStatus::Complete,
				blocked_reason: None,
				completed_at: Some(Utc::now()),
			},
			Task {
				id: "task-2".to_string(),
				title: "Task 2".to_string(),
				description: "Second task".to_string(),
				acceptance_criteria: vec![],
				status: TaskStatus::Pending,
				blocked_reason: None,
				completed_at: None,
			},
		],
		learnings: vec![],
	};

	let next = spec.find_next_task();
	assert!(next.is_some());
	assert_eq!(next.unwrap().id, "task-2");
}

#[test]
fn test_find_next_task_skips_blocked() {
	let spec = Spec {
		name: "test".to_string(),
		description: "Test".to_string(),
		branch_name: "feature/test".to_string(),
		created_at: Utc::now(),
		tasks: vec![
			Task {
				id: "task-1".to_string(),
				title: "Task 1".to_string(),
				description: "First task".to_string(),
				acceptance_criteria: vec![],
				status: TaskStatus::Blocked,
				blocked_reason: Some("Missing dep".to_string()),
				completed_at: None,
			},
			Task {
				id: "task-2".to_string(),
				title: "Task 2".to_string(),
				description: "Second task".to_string(),
				acceptance_criteria: vec![],
				status: TaskStatus::Pending,
				blocked_reason: None,
				completed_at: None,
			},
		],
		learnings: vec![],
	};

	let next = spec.find_next_task();
	assert!(next.is_some());
	assert_eq!(next.unwrap().id, "task-2");
}

#[test]
fn test_find_next_task_skips_in_progress() {
	let spec = Spec {
		name: "test".to_string(),
		description: "Test".to_string(),
		branch_name: "feature/test".to_string(),
		created_at: Utc::now(),
		tasks: vec![
			Task {
				id: "task-1".to_string(),
				title: "Task 1".to_string(),
				description: "First task".to_string(),
				acceptance_criteria: vec![],
				status: TaskStatus::InProgress,
				blocked_reason: None,
				completed_at: None,
			},
			Task {
				id: "task-2".to_string(),
				title: "Task 2".to_string(),
				description: "Second task".to_string(),
				acceptance_criteria: vec![],
				status: TaskStatus::Pending,
				blocked_reason: None,
				completed_at: None,
			},
		],
		learnings: vec![],
	};

	let next = spec.find_next_task();
	assert!(next.is_some());
	assert_eq!(next.unwrap().id, "task-2");
}

#[test]
fn test_find_next_task_returns_none_when_all_complete() {
	let spec = Spec {
		name: "test".to_string(),
		description: "Test".to_string(),
		branch_name: "feature/test".to_string(),
		created_at: Utc::now(),
		tasks: vec![
			Task {
				id: "task-1".to_string(),
				title: "Task 1".to_string(),
				description: "First task".to_string(),
				acceptance_criteria: vec![],
				status: TaskStatus::Complete,
				blocked_reason: None,
				completed_at: Some(Utc::now()),
			},
		],
		learnings: vec![],
	};

	let next = spec.find_next_task();
	assert!(next.is_none());
}

#[test]
fn test_add_learning() {
	let mut spec = Spec {
		name: "test".to_string(),
		description: "Test".to_string(),
		branch_name: "feature/test".to_string(),
		created_at: Utc::now(),
		tasks: vec![],
		learnings: vec![],
	};

	spec.add_learning("First learning".to_string());
	spec.add_learning("Second learning".to_string());

	assert_eq!(spec.learnings.len(), 2);
	assert_eq!(spec.learnings[0], "First learning");
	assert_eq!(spec.learnings[1], "Second learning");
}

#[test]
fn test_find_task_mut() {
	let mut spec = Spec {
		name: "test".to_string(),
		description: "Test".to_string(),
		branch_name: "feature/test".to_string(),
		created_at: Utc::now(),
		tasks: vec![
			Task {
				id: "task-1".to_string(),
				title: "Task 1".to_string(),
				description: "First task".to_string(),
				acceptance_criteria: vec![],
				status: TaskStatus::Pending,
				blocked_reason: None,
				completed_at: None,
			},
		],
		learnings: vec![],
	};

	let task = spec.find_task_mut("task-1");
	assert!(task.is_some());

	let task = task.unwrap();
	task.status = TaskStatus::Complete;
	task.completed_at = Some(Utc::now());

	assert_eq!(spec.tasks[0].status, TaskStatus::Complete);
}
```

**Step 2: Run tests**

Run: `cargo test`
Expected: All tests pass

**Step 3: Commit**

```bash
git add tests/ralph_test.rs
git commit -m "test: add comprehensive unit tests for spec and ralph loop logic"
```

---

### Task 14: Fix Instruction Mismatch in Ralph Context

**Files:**
- Modify: `src/ralph/mod.rs`

**Step 1: Fix the tool name mismatch**

In `src/ralph/mod.rs`, in `build_context()`, change:
```rust
context.push_str("4. Use shell_command to run tests, builds, git commands\n");
```

To:
```rust
context.push_str("4. Use run_command to run tests, builds, git commands\n");
```

**Step 2: Run tests**

Run: `cargo test`
Expected: All tests pass

**Step 3: Commit**

```bash
git add src/ralph/mod.rs
git commit -m "fix: correct tool name in Ralph context instructions (shell_command -> run_command)"
```

---

### Task 15: Update Documentation

**Files:**
- Modify: `rustagent.toml.example`
- Modify: `README.md`
- Modify: `CLAUDE.md`

**Step 1: Update example config**

Update `rustagent.toml.example` to include all providers:
```toml
# Rustagent Configuration
# Copy this file to rustagent.toml and fill in your API keys

# Default LLM configuration (fallback for all modes)
[llm]
provider = "anthropic"  # Options: "anthropic", "openai", "ollama"
model = "claude-sonnet-4-20250514"
max_tokens = 8192

# Provider-specific configuration
[anthropic]
api_key = "${ANTHROPIC_API_KEY}"

[openai]
api_key = "${OPENAI_API_KEY}"

[ollama]
base_url = "http://localhost:11434"

# Optional: Override LLM for planning mode
# [planning.llm]
# provider = "anthropic"
# model = "claude-opus-4-20250514"
# max_tokens = 16384

# Optional: Override LLM for ralph mode
# [ralph.llm]
# provider = "anthropic"
# model = "claude-sonnet-4-20250514"
# max_tokens = 4096

# Agent configuration
[rustagent]
spec_dir = "specs"
max_iterations = 100  # Set to null for unlimited

# Security configuration
[security]
shell_policy = "allowlist"  # Options: "allowlist", "blocklist", "unrestricted"
allowed_commands = ["git", "cargo", "npm", "pnpm", "yarn", "ls", "cat", "grep", "find", "echo", "pwd", "mkdir", "touch"]
blocked_patterns = []  # Regex patterns to block (used with blocklist policy)
max_file_size_mb = 10
allowed_paths = ["."]  # Paths the agent can access
```

**Step 2: Update README with new features**

Add sections for:
- OpenAI and Ollama provider support
- Logging location
- Signal completion tool

**Step 3: Update CLAUDE.md**

Update with accurate commands and remove mention of 2021 edition.

**Step 4: Commit**

```bash
git add rustagent.toml.example README.md CLAUDE.md
git commit -m "docs: update documentation for new providers, logging, and features"
```

---

## Final Steps

### Task 16: Final Verification

**Step 1: Run all checks**

Run:
```bash
cargo fmt
cargo clippy
cargo test
cargo build --release
```

Expected: All pass with no warnings

**Step 2: Verify logging works**

Run:
```bash
cargo run -- init
ls -la ~/.local/state/rustagent/logs/
```

Expected: Log file exists

**Step 3: Commit any final fixes**

```bash
git add -A
git commit -m "chore: final cleanup and verification"
```

---

## Summary

This plan addresses all identified issues across 16 tasks:

**Phase 1: Foundation Fixes (Tasks 1-5)**
- ✅ Extended Message model with Role::Tool and tool_call_id for OpenAI compatibility
- ✅ Unified error handling - LlmClient uses anyhow::Result
- ✅ Eliminated code duplication - LLM factory, tools factory, permission checker

**Phase 2: Logging (Task 6)**
- ✅ File-based logging with tracing - XDG-compliant, daily rotation, early for debugging

**Phase 3: Providers (Tasks 7-8)**
- ✅ OpenAI provider - Full tool calling with proper tool_call_id handling
- ✅ Ollama provider - Defensive parsing for version variations

**Phase 4: CLI & Tools (Tasks 9-10)**
- ✅ Init command - Creates spec dir and config template
- ✅ Signal completion tool - Structured completion instead of magic strings

**Phase 5: Polish & Testing (Tasks 11-16)**
- ✅ Fixed permission handler - Returns error instead of process::exit
- ✅ Mock LLM client - For testing agent behavior
- ✅ Comprehensive tests - Spec logic, providers, tools
- ✅ Fixed instruction mismatch - Correct tool names
- ✅ Updated documentation - All features documented

**Key Oracle Recommendations Incorporated:**
1. Message model extended first (before providers)
2. Logging moved early (before provider debugging)
3. Init tests use `env!("CARGO_BIN_EXE_rustagent")` instead of cargo shell-out
4. Ollama defensive parsing for argument string vs object
5. Proper WorkerGuard lifetime for tracing-appender
