# Rustagent Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a CLI-based AI agent system using the Ralph Loop pattern with planning and autonomous execution modes.

**Architecture:** Trait-based LLM abstraction supporting multiple providers (Anthropic, OpenAI, Ollama), modular tool system, JSON-based spec persistence, and Ralph Loop algorithm with fresh context per iteration.

**Tech Stack:** Rust (2021 edition), tokio async runtime, clap CLI, reqwest HTTP, serde JSON, anyhow error handling, toml config parsing

---

## Pre-Implementation: Fix Edition

### Task 0: Fix Cargo.toml Edition

**Files:**
- Modify: `Cargo.toml:4`

**Step 1: Update edition from 2024 to 2021**

Change line 4 in `Cargo.toml` from:
```toml
edition = "2024"
```

To:
```toml
edition = "2021"
```

**Step 2: Verify the fix**

Run: `cargo check`
Expected: Compilation succeeds without edition warning

**Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "fix: correct Rust edition from 2024 to 2021"
```

---

## Phase 1: Core Infrastructure

### Task 1: Configuration System

**Files:**
- Create: `src/config.rs`
- Modify: `src/main.rs`
- Modify: `Cargo.toml`

**Step 1: Write tests for config loading**

Create `tests/config_test.rs`:

```rust
use rustagent::config::{Config, LlmProvider};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_config_from_toml() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("config.toml");

    fs::write(&config_path, r#"
[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"

[anthropic]
api_key = "test-key"

[rustagent]
spec_dir = "specs"
"#).unwrap();

    let config = Config::load(&config_path).unwrap();
    assert_eq!(config.llm.provider, LlmProvider::Anthropic);
    assert_eq!(config.llm.model, "claude-sonnet-4-20250514");
}

#[test]
fn test_config_env_var_substitution() {
    std::env::set_var("TEST_API_KEY", "secret123");

    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("config.toml");

    fs::write(&config_path, r#"
[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"

[anthropic]
api_key = "${TEST_API_KEY}"
"#).unwrap();

    let config = Config::load(&config_path).unwrap();
    assert_eq!(config.anthropic.as_ref().unwrap().api_key, "secret123");

    std::env::remove_var("TEST_API_KEY");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test test_config`
Expected: FAIL with "no `config` module in `rustagent`"

**Step 3: Add dependencies**

Add to `Cargo.toml` [dependencies]:
```toml
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"
tokio = { version = "1.43", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }

[dev-dependencies]
tempfile = "3.15"
```

**Step 4: Implement config module**

Create `src/config.rs`:

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Anthropic,
    OpenAi,
    Ollama,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicConfig {
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiConfig {
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustagentConfig {
    pub spec_dir: String,
    pub max_iterations: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub llm: LlmConfig,
    pub anthropic: Option<AnthropicConfig>,
    pub openai: Option<OpenAiConfig>,
    pub ollama: Option<OllamaConfig>,
    pub rustagent: RustagentConfig,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .context("Failed to read config file")?;

        // Expand environment variables
        let expanded = Self::expand_env_vars(&content);

        let config: Config = toml::from_str(&expanded)
            .context("Failed to parse config file")?;

        Ok(config)
    }

    fn expand_env_vars(content: &str) -> String {
        let mut result = content.to_string();

        // Find all ${VAR} patterns
        while let Some(start) = result.find("${") {
            if let Some(end) = result[start..].find('}') {
                let var_name = &result[start + 2..start + end];
                let value = std::env::var(var_name).unwrap_or_default();
                result.replace_range(start..start + end + 1, &value);
            } else {
                break;
            }
        }

        result
    }
}

impl Default for RustagentConfig {
    fn default() -> Self {
        Self {
            spec_dir: "specs".to_string(),
            max_iterations: None,
        }
    }
}
```

**Step 5: Export config module**

Modify `src/main.rs` to add at the top:
```rust
pub mod config;
```

**Step 6: Run tests to verify they pass**

Run: `cargo test test_config`
Expected: PASS - both config tests succeed

**Step 7: Commit**

```bash
git add src/config.rs src/main.rs Cargo.toml Cargo.lock tests/config_test.rs
git commit -m "feat: add configuration system with TOML parsing and env var expansion"
```

---

### Task 2: LLM Client Trait and Types

**Files:**
- Create: `src/llm/mod.rs`
- Modify: `src/main.rs`

**Step 1: Write test for message serialization**

Create `tests/llm_test.rs`:

```rust
use rustagent::llm::{Message, Role, ToolDefinition, ToolResult};
use serde_json::json;

#[test]
fn test_message_serialization() {
    let msg = Message {
        role: Role::User,
        content: "Hello".to_string(),
    };

    let json = serde_json::to_value(&msg).unwrap();
    assert_eq!(json["role"], "user");
    assert_eq!(json["content"], "Hello");
}

#[test]
fn test_tool_definition_parameters() {
    let tool = ToolDefinition {
        name: "read_file".to_string(),
        description: "Read a file".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"}
            },
            "required": ["path"]
        }),
    };

    assert_eq!(tool.name, "read_file");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_message`
Expected: FAIL with "no `llm` module"

**Step 3: Create LLM module directory**

Run: `mkdir -p src/llm`

**Step 4: Implement LLM types**

Create `src/llm/mod.rs`:

```rust
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
}

#[derive(Debug, Clone)]
pub enum ResponseContent {
    Text(String),
    ToolCalls(Vec<ToolCall>),
}

#[derive(Debug, Clone)]
pub struct Response {
    pub content: ResponseContent,
    pub stop_reason: Option<String>,
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat(
        &self,
        messages: Vec<Message>,
        tools: &[ToolDefinition],
    ) -> Result<Response>;
}
```

**Step 5: Export LLM module and add dependency**

Modify `src/main.rs` to add after config module:
```rust
pub mod llm;
```

Add to `Cargo.toml` [dependencies]:
```toml
async-trait = "0.1"
```

**Step 6: Run tests to verify they pass**

Run: `cargo test test_message`
Expected: PASS - both serialization tests succeed

**Step 7: Commit**

```bash
git add src/llm/ src/main.rs Cargo.toml Cargo.lock tests/llm_test.rs
git commit -m "feat: add LLM client trait and core types"
```

---

### Task 3: Anthropic Client Implementation

**Files:**
- Create: `src/llm/anthropic.rs`
- Modify: `src/llm/mod.rs`

**Step 1: Write test for Anthropic client**

Add to `tests/llm_test.rs`:

```rust
use rustagent::llm::anthropic::AnthropicClient;
use rustagent::llm::LlmClient;

#[tokio::test]
async fn test_anthropic_message_format() {
    // This test validates request structure, doesn't actually call API
    let client = AnthropicClient::new("test-key".to_string());

    let messages = vec![
        Message {
            role: Role::User,
            content: "Hello".to_string(),
        }
    ];

    // We'll test this by mocking in future, for now just construct
    assert!(client.format_request(&messages, &[]).is_ok());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_anthropic`
Expected: FAIL with "no `anthropic` module"

**Step 3: Implement Anthropic client**

Create `src/llm/anthropic.rs`:

```rust
use super::{LlmClient, Message, Response, ResponseContent, Role, ToolCall, ToolDefinition};
use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicClient {
    api_key: String,
    client: Client,
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
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
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
        }
    }

    pub fn format_request(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<serde_json::Value> {
        let anthropic_messages: Vec<AnthropicMessage> = messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(|m| AnthropicMessage {
                role: match m.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                    Role::System => "user".to_string(),
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
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 8192,
            messages: anthropic_messages,
            tools: anthropic_tools,
        };

        Ok(serde_json::to_value(request)?)
    }
}

#[async_trait]
impl LlmClient for AnthropicClient {
    async fn chat(&self, messages: Vec<Message>, tools: &[ToolDefinition]) -> Result<Response> {
        let request_body = self.format_request(&messages, tools)?;

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to send request to Anthropic API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error {}: {}", status, body);
        }

        let anthropic_response: AnthropicResponse = response
            .json()
            .await
            .context("Failed to parse Anthropic response")?;

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
```

**Step 4: Export Anthropic module**

Modify `src/llm/mod.rs` to add at the bottom:
```rust
pub mod anthropic;
```

**Step 5: Run tests to verify they pass**

Run: `cargo test test_anthropic`
Expected: PASS - Anthropic format test succeeds

**Step 6: Commit**

```bash
git add src/llm/anthropic.rs src/llm/mod.rs tests/llm_test.rs
git commit -m "feat: implement Anthropic API client"
```

---

## Phase 2: Tool System

### Task 4: Tool Trait and Registry

**Files:**
- Create: `src/tools/mod.rs`
- Modify: `src/main.rs`

**Step 1: Write test for tool registry**

Create `tests/tools_test.rs`:

```rust
use rustagent::tools::{Tool, ToolRegistry};
use async_trait::async_trait;
use anyhow::Result;

struct MockTool;

#[async_trait]
impl Tool for MockTool {
    fn name(&self) -> &str {
        "mock_tool"
    }

    fn description(&self) -> &str {
        "A mock tool"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _params: serde_json::Value) -> Result<String> {
        Ok("success".to_string())
    }
}

#[tokio::test]
async fn test_tool_registry() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(MockTool));

    assert!(registry.get("mock_tool").is_some());
    assert!(registry.get("nonexistent").is_none());
}

#[tokio::test]
async fn test_tool_execute() {
    let tool = MockTool;
    let result = tool.execute(serde_json::json!({})).await.unwrap();
    assert_eq!(result, "success");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_tool`
Expected: FAIL with "no `tools` module"

**Step 3: Create tools module directory**

Run: `mkdir -p src/tools`

**Step 4: Implement tool trait and registry**

Create `src/tools/mod.rs`:

```rust
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;
    async fn execute(&self, params: serde_json::Value) -> Result<String>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<&Box<dyn Tool>> {
        self.tools.get(name)
    }

    pub fn list(&self) -> Vec<&Box<dyn Tool>> {
        self.tools.values().collect()
    }

    pub fn definitions(&self) -> Vec<crate::llm::ToolDefinition> {
        self.tools
            .values()
            .map(|tool| crate::llm::ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.parameters(),
            })
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 5: Export tools module**

Modify `src/main.rs` to add after llm module:
```rust
pub mod tools;
```

**Step 6: Run tests to verify they pass**

Run: `cargo test test_tool`
Expected: PASS - both tool registry tests succeed

**Step 7: Commit**

```bash
git add src/tools/ src/main.rs tests/tools_test.rs
git commit -m "feat: add tool trait and registry system"
```

---

### Task 5: File Tools Implementation

**Files:**
- Create: `src/tools/file.rs`
- Modify: `src/tools/mod.rs`

**Step 1: Write tests for file tools**

Add to `tests/tools_test.rs`:

```rust
use rustagent::tools::file::{ReadFileTool, WriteFileTool, ListFilesTool};
use serde_json::json;
use tempfile::TempDir;
use std::fs;

#[tokio::test]
async fn test_read_file_tool() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("test.txt");
    fs::write(&file_path, "hello world").unwrap();

    let tool = ReadFileTool;
    let result = tool.execute(json!({
        "path": file_path.to_str().unwrap()
    })).await.unwrap();

    assert!(result.contains("hello world"));
}

#[tokio::test]
async fn test_write_file_tool() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("output.txt");

    let tool = WriteFileTool;
    tool.execute(json!({
        "path": file_path.to_str().unwrap(),
        "content": "test content"
    })).await.unwrap();

    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "test content");
}

#[tokio::test]
async fn test_list_files_tool() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("file1.txt"), "a").unwrap();
    fs::write(temp.path().join("file2.txt"), "b").unwrap();

    let tool = ListFilesTool;
    let result = tool.execute(json!({
        "path": temp.path().to_str().unwrap()
    })).await.unwrap();

    assert!(result.contains("file1.txt"));
    assert!(result.contains("file2.txt"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test test_read_file_tool test_write_file_tool test_list_files_tool`
Expected: FAIL with "no `file` module in `tools`"

**Step 3: Implement file tools**

Create `src/tools/file.rs`:

```rust
use super::Tool;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::path::Path;

#[derive(Deserialize)]
struct ReadFileParams {
    path: String,
}

pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> Result<String> {
        let params: ReadFileParams = serde_json::from_value(params)
            .context("Invalid parameters for read_file")?;

        let content = tokio::fs::read_to_string(&params.path)
            .await
            .context(format!("Failed to read file: {}", params.path))?;

        Ok(content)
    }
}

#[derive(Deserialize)]
struct WriteFileParams {
    path: String,
    content: String,
}

pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file, creating it if it doesn't exist"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> Result<String> {
        let params: WriteFileParams = serde_json::from_value(params)
            .context("Invalid parameters for write_file")?;

        // Create parent directories if they don't exist
        if let Some(parent) = Path::new(&params.path).parent() {
            tokio::fs::create_dir_all(parent).await.context("Failed to create parent directories")?;
        }

        tokio::fs::write(&params.path, &params.content)
            .await
            .context(format!("Failed to write file: {}", params.path))?;

        Ok(format!("Successfully wrote {} bytes to {}", params.content.len(), params.path))
    }
}

#[derive(Deserialize)]
struct ListFilesParams {
    path: String,
}

pub struct ListFilesTool;

#[async_trait]
impl Tool for ListFilesTool {
    fn name(&self) -> &str {
        "list_files"
    }

    fn description(&self) -> &str {
        "List files and directories in a given path"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to list files from"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> Result<String> {
        let params: ListFilesParams = serde_json::from_value(params)
            .context("Invalid parameters for list_files")?;

        let mut entries = tokio::fs::read_dir(&params.path)
            .await
            .context(format!("Failed to read directory: {}", params.path))?;

        let mut result = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let is_dir = path.is_dir();
            result.push(format!("{}{}", name, if is_dir { "/" } else { "" }));
        }

        result.sort();
        Ok(result.join("\n"))
    }
}
```

**Step 4: Export file tools**

Modify `src/tools/mod.rs` to add at the bottom:
```rust
pub mod file;
```

**Step 5: Run tests to verify they pass**

Run: `cargo test test_read_file_tool test_write_file_tool test_list_files_tool`
Expected: PASS - all three file tool tests succeed

**Step 6: Commit**

```bash
git add src/tools/file.rs src/tools/mod.rs tests/tools_test.rs
git commit -m "feat: implement file operation tools (read, write, list)"
```

---

### Task 6: Shell Command Tool

**Files:**
- Create: `src/tools/shell.rs`
- Modify: `src/tools/mod.rs`

**Step 1: Write test for shell tool**

Add to `tests/tools_test.rs`:

```rust
use rustagent::tools::shell::RunCommandTool;

#[tokio::test]
async fn test_run_command_tool() {
    let tool = RunCommandTool;
    let result = tool.execute(json!({
        "command": "echo hello"
    })).await.unwrap();

    assert!(result.contains("hello"));
}

#[tokio::test]
async fn test_run_command_with_working_dir() {
    let temp = TempDir::new().unwrap();

    let tool = RunCommandTool;
    let result = tool.execute(json!({
        "command": "pwd",
        "working_dir": temp.path().to_str().unwrap()
    })).await.unwrap();

    assert!(result.contains(temp.path().to_str().unwrap()));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test test_run_command`
Expected: FAIL with "no `shell` module in `tools`"

**Step 3: Implement shell tool**

Create `src/tools/shell.rs`:

```rust
use super::Tool;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use tokio::process::Command;

#[derive(Deserialize)]
struct RunCommandParams {
    command: String,
    working_dir: Option<String>,
}

pub struct RunCommandTool;

#[async_trait]
impl Tool for RunCommandTool {
    fn name(&self) -> &str {
        "run_command"
    }

    fn description(&self) -> &str {
        "Execute a shell command and return its output"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Optional working directory for the command"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> Result<String> {
        let params: RunCommandParams = serde_json::from_value(params)
            .context("Invalid parameters for run_command")?;

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", &params.command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", &params.command]);
            c
        };

        if let Some(working_dir) = params.working_dir {
            cmd.current_dir(working_dir);
        }

        let output = cmd
            .output()
            .await
            .context("Failed to execute command")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut result = String::new();
        if !stdout.is_empty() {
            result.push_str("STDOUT:\n");
            result.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push_str("\n");
            }
            result.push_str("STDERR:\n");
            result.push_str(&stderr);
        }

        if !output.status.success() {
            result.push_str(&format!("\nExit code: {}", output.status.code().unwrap_or(-1)));
        }

        Ok(result)
    }
}
```

**Step 4: Export shell module**

Modify `src/tools/mod.rs` to add at the bottom:
```rust
pub mod shell;
```

**Step 5: Run tests to verify they pass**

Run: `cargo test test_run_command`
Expected: PASS - both shell command tests succeed

**Step 6: Commit**

```bash
git add src/tools/shell.rs src/tools/mod.rs tests/tools_test.rs
git commit -m "feat: implement shell command execution tool"
```

---

## Phase 3: Spec System

### Task 7: Spec Data Structure

**Files:**
- Create: `src/spec.rs`
- Modify: `src/main.rs`

**Step 1: Write tests for spec serialization**

Create `tests/spec_test.rs`:

```rust
use rustagent::spec::{Spec, Task, TaskStatus};
use tempfile::TempDir;
use std::fs;

#[test]
fn test_spec_serialization() {
    let spec = Spec {
        name: "test-feature".to_string(),
        description: "A test feature".to_string(),
        branch_name: "feature/test".to_string(),
        created_at: "2026-01-19T12:00:00Z".to_string(),
        tasks: vec![
            Task {
                id: "task-1".to_string(),
                title: "Implement X".to_string(),
                description: "Description".to_string(),
                acceptance_criteria: vec!["Criterion 1".to_string()],
                status: TaskStatus::Pending,
                blocked_reason: None,
                completed_at: None,
            }
        ],
        learnings: vec![],
    };

    let json = serde_json::to_string_pretty(&spec).unwrap();
    assert!(json.contains("test-feature"));
    assert!(json.contains("pending"));
}

#[test]
fn test_spec_save_and_load() {
    let temp = TempDir::new().unwrap();
    let spec_path = temp.path().join("test.json");

    let spec = Spec {
        name: "test".to_string(),
        description: "Test".to_string(),
        branch_name: "feature/test".to_string(),
        created_at: "2026-01-19T12:00:00Z".to_string(),
        tasks: vec![],
        learnings: vec![],
    };

    spec.save(&spec_path).unwrap();
    let loaded = Spec::load(&spec_path).unwrap();

    assert_eq!(loaded.name, "test");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test test_spec`
Expected: FAIL with "no `spec` module"

**Step 3: Implement spec module**

Create `src/spec.rs`:

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Complete,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub acceptance_criteria: Vec<String>,
    pub status: TaskStatus,
    pub blocked_reason: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spec {
    pub name: String,
    pub description: String,
    pub branch_name: String,
    pub created_at: String,
    pub tasks: Vec<Task>,
    pub learnings: Vec<String>,
}

impl Spec {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .context("Failed to read spec file")?;

        let spec: Spec = serde_json::from_str(&content)
            .context("Failed to parse spec file")?;

        Ok(spec)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize spec")?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create spec directory")?;
        }

        std::fs::write(path, json)
            .context("Failed to write spec file")?;

        Ok(())
    }

    pub fn find_next_task(&self) -> Option<&Task> {
        self.tasks.iter().find(|t| t.status == TaskStatus::Pending)
    }

    pub fn find_task_mut(&mut self, task_id: &str) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == task_id)
    }

    pub fn add_learning(&mut self, learning: String) {
        self.learnings.push(learning);
    }
}
```

**Step 4: Export spec module**

Modify `src/main.rs` to add after tools module:
```rust
pub mod spec;
```

**Step 5: Run tests to verify they pass**

Run: `cargo test test_spec`
Expected: PASS - both spec tests succeed

**Step 6: Commit**

```bash
git add src/spec.rs src/main.rs tests/spec_test.rs
git commit -m "feat: add spec data structure with JSON persistence"
```

---

## Phase 4: CLI Commands

### Task 8: CLI Structure with Subcommands

**Files:**
- Modify: `src/main.rs`

**Step 1: Write integration test for CLI**

Create `tests/cli_test.rs`:

```rust
use std::process::Command;

#[test]
fn test_cli_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "--help"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rustagent"));
    assert!(stdout.contains("init"));
    assert!(stdout.contains("plan"));
    assert!(stdout.contains("run"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_cli_help`
Expected: FAIL - current CLI doesn't have subcommands

**Step 3: Implement CLI structure**

Replace `src/main.rs` content with:

```rust
use clap::{Parser, Subcommand};
use clap_verbosity_flag::Verbosity;
use log::info;

pub mod config;
pub mod llm;
pub mod tools;
pub mod spec;

#[derive(Debug, Parser)]
#[command(name = "rustagent")]
#[command(about = "A Rust implementation of the Ralph Loop pattern for AI agents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[command(flatten)]
    verbosity: Verbosity,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Initialize spec directory
    Init {
        /// Spec directory path
        #[arg(long, default_value = "specs")]
        spec_dir: String,
    },

    /// Interactive planning mode
    Plan {
        /// Spec directory path
        #[arg(long, default_value = "specs")]
        spec_dir: String,
    },

    /// Run Ralph loop on a spec file
    Run {
        /// Path to the spec file
        spec_file: String,

        /// Maximum number of iterations
        #[arg(long)]
        max_iterations: Option<usize>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    env_logger::Builder::new()
        .filter_level(args.verbosity.log_level_filter())
        .init();

    match args.command {
        Commands::Init { spec_dir } => {
            info!("Initializing spec directory: {}", spec_dir);
            std::fs::create_dir_all(&spec_dir)?;
            println!("Created spec directory: {}", spec_dir);
            Ok(())
        }

        Commands::Plan { spec_dir } => {
            info!("Starting planning mode");
            println!("Planning mode not yet implemented");
            println!("Spec directory: {}", spec_dir);
            Ok(())
        }

        Commands::Run { spec_file, max_iterations } => {
            info!("Running Ralph loop on: {}", spec_file);
            println!("Ralph loop not yet implemented");
            println!("Spec file: {}", spec_file);
            if let Some(max) = max_iterations {
                println!("Max iterations: {}", max);
            }
            Ok(())
        }
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test test_cli_help`
Expected: PASS - CLI help shows subcommands

**Step 5: Test CLI manually**

Run: `cargo run -- --help`
Expected: Shows help with init, plan, and run subcommands

**Step 6: Commit**

```bash
git add src/main.rs tests/cli_test.rs
git commit -m "feat: add CLI structure with init, plan, and run subcommands"
```

---

## Phase 5: Planning Agent

### Task 9: Planning Agent Implementation

**Files:**
- Create: `src/planning/mod.rs`
- Modify: `src/main.rs`

**Step 1: Write test for planning agent**

Create `tests/planning_test.rs`:

```rust
use rustagent::planning::PlanningAgent;
use rustagent::config::Config;
use tempfile::TempDir;
use std::fs;

#[tokio::test]
async fn test_planning_agent_creation() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("config.toml");

    fs::write(&config_path, r#"
[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"

[anthropic]
api_key = "test-key"

[rustagent]
spec_dir = "specs"
"#).unwrap();

    let config = Config::load(&config_path).unwrap();
    let agent = PlanningAgent::new(config, temp.path().to_str().unwrap().to_string());

    assert!(agent.spec_dir.ends_with(temp.path().to_str().unwrap()));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_planning_agent`
Expected: FAIL with "no `planning` module"

**Step 3: Create planning module directory**

Run: `mkdir -p src/planning`

**Step 4: Implement planning agent skeleton**

Create `src/planning/mod.rs`:

```rust
use crate::config::Config;
use crate::llm::{LlmClient, Message, Role};
use crate::llm::anthropic::AnthropicClient;
use crate::tools::{Tool, ToolRegistry};
use crate::tools::file::{ReadFileTool, WriteFileTool, ListFilesTool};
use crate::tools::shell::RunCommandTool;
use crate::spec::{Spec, Task, TaskStatus};
use anyhow::{Context, Result};
use std::io::{self, Write};

pub struct PlanningAgent {
    client: Box<dyn LlmClient>,
    tools: ToolRegistry,
    pub spec_dir: String,
    conversation: Vec<Message>,
}

impl PlanningAgent {
    pub fn new(config: Config, spec_dir: String) -> Self {
        let client: Box<dyn LlmClient> = match config.llm.provider {
            crate::config::LlmProvider::Anthropic => {
                let api_key = config
                    .anthropic
                    .expect("Anthropic config required")
                    .api_key;
                Box::new(AnthropicClient::new(api_key))
            }
            _ => panic!("Only Anthropic provider supported in planning mode"),
        };

        let mut tools = ToolRegistry::new();
        tools.register(Box::new(ReadFileTool));
        tools.register(Box::new(WriteFileTool));
        tools.register(Box::new(ListFilesTool));
        tools.register(Box::new(RunCommandTool));

        let system_prompt = r#"You are a planning agent helping to break down software projects into discrete tasks.

Your role is to:
1. Ask clarifying questions one at a time
2. Research the codebase using available tools
3. Break work into small, discrete tasks
4. Create a detailed spec in JSON format

Be thorough but concise. Each task should be independently achievable."#;

        let conversation = vec![Message {
            role: Role::System,
            content: system_prompt.to_string(),
        }];

        Self {
            client,
            tools,
            spec_dir,
            conversation,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        println!("Planning Agent - Interactive Mode");
        println!("Describe what you want to build, and I'll help create a detailed spec.\n");

        loop {
            print!("> ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            if input == "exit" || input == "quit" {
                break;
            }

            self.conversation.push(Message {
                role: Role::User,
                content: input.to_string(),
            });

            let response = self.process_turn().await?;
            println!("\n{}\n", response);
        }

        Ok(())
    }

    async fn process_turn(&mut self) -> Result<String> {
        let tool_defs = self.tools.definitions();
        let response = self.client.chat(self.conversation.clone(), &tool_defs).await?;

        match response.content {
            crate::llm::ResponseContent::Text(text) => {
                self.conversation.push(Message {
                    role: Role::Assistant,
                    content: text.clone(),
                });
                Ok(text)
            }
            crate::llm::ResponseContent::ToolCalls(calls) => {
                let mut results = Vec::new();

                for call in calls {
                    if let Some(tool) = self.tools.get(&call.name) {
                        let result = tool.execute(call.parameters).await?;
                        results.push(format!("Tool {}: {}", call.name, result));
                    }
                }

                let result_text = results.join("\n\n");
                self.conversation.push(Message {
                    role: Role::User,
                    content: format!("Tool results:\n{}", result_text),
                });

                // Recurse to get the agent's response after tool use
                self.process_turn().await
            }
        }
    }
}
```

**Step 5: Export planning module**

Modify `src/main.rs` to add after spec module:
```rust
pub mod planning;
```

**Step 6: Update plan command to use planning agent**

In `src/main.rs`, replace the `Commands::Plan` handler:

```rust
Commands::Plan { spec_dir } => {
    info!("Starting planning mode");

    // Load config
    let config_path = dirs::home_dir()
        .map(|h| h.join(".config/rustagent/config.toml"))
        .unwrap_or_else(|| "rustagent.toml".into());

    let config = if config_path.exists() {
        config::Config::load(&config_path)?
    } else {
        anyhow::bail!("Config file not found. Create ~/.config/rustagent/config.toml or rustagent.toml");
    };

    let mut agent = planning::PlanningAgent::new(config, spec_dir);
    agent.run().await?;

    Ok(())
}
```

Add dependency to `Cargo.toml`:
```toml
dirs = "5.0"
```

**Step 7: Run test to verify it passes**

Run: `cargo test test_planning_agent`
Expected: PASS - planning agent can be created

**Step 8: Commit**

```bash
git add src/planning/ src/main.rs tests/planning_test.rs Cargo.toml Cargo.lock
git commit -m "feat: implement planning agent with interactive conversation"
```

---

## Phase 6: Ralph Loop

### Task 10: Ralph Loop Implementation

**Files:**
- Create: `src/ralph/mod.rs`
- Modify: `src/main.rs`

**Step 1: Write test for Ralph loop**

Create `tests/ralph_test.rs`:

```rust
use rustagent::ralph::RalphLoop;
use rustagent::config::Config;
use rustagent::spec::{Spec, Task, TaskStatus};
use tempfile::TempDir;
use std::fs;

#[tokio::test]
async fn test_ralph_loop_creation() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("config.toml");
    let spec_path = temp.path().join("test.json");

    fs::write(&config_path, r#"
[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"

[anthropic]
api_key = "test-key"

[rustagent]
spec_dir = "specs"
"#).unwrap();

    let spec = Spec {
        name: "test".to_string(),
        description: "Test".to_string(),
        branch_name: "feature/test".to_string(),
        created_at: "2026-01-19T12:00:00Z".to_string(),
        tasks: vec![],
        learnings: vec![],
    };
    spec.save(&spec_path).unwrap();

    let config = Config::load(&config_path).unwrap();
    let ralph = RalphLoop::new(config, spec_path.to_str().unwrap().to_string(), None);

    assert!(ralph.spec_path.ends_with("test.json"));
}

#[test]
fn test_find_next_pending_task() {
    let spec = Spec {
        name: "test".to_string(),
        description: "Test".to_string(),
        branch_name: "feature/test".to_string(),
        created_at: "2026-01-19T12:00:00Z".to_string(),
        tasks: vec![
            Task {
                id: "task-1".to_string(),
                title: "Task 1".to_string(),
                description: "First task".to_string(),
                acceptance_criteria: vec![],
                status: TaskStatus::Complete,
                blocked_reason: None,
                completed_at: Some("2026-01-19T13:00:00Z".to_string()),
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
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_ralph`
Expected: FAIL with "no `ralph` module"

**Step 3: Create ralph module directory**

Run: `mkdir -p src/ralph`

**Step 4: Implement Ralph loop**

Create `src/ralph/mod.rs`:

```rust
use crate::config::Config;
use crate::llm::{LlmClient, Message, Role, ResponseContent};
use crate::llm::anthropic::AnthropicClient;
use crate::tools::{Tool, ToolRegistry};
use crate::tools::file::{ReadFileTool, WriteFileTool, ListFilesTool};
use crate::tools::shell::RunCommandTool;
use crate::spec::{Spec, TaskStatus};
use anyhow::{Context, Result};
use log::{info, warn};

const DEFAULT_ITERATION_DELAY_MS: u64 = 2000;

pub struct RalphLoop {
    client: Box<dyn LlmClient>,
    tools: ToolRegistry,
    pub spec_path: String,
    max_iterations: Option<usize>,
}

impl RalphLoop {
    pub fn new(config: Config, spec_path: String, max_iterations: Option<usize>) -> Self {
        let client: Box<dyn LlmClient> = match config.llm.provider {
            crate::config::LlmProvider::Anthropic => {
                let api_key = config
                    .anthropic
                    .expect("Anthropic config required")
                    .api_key;
                Box::new(AnthropicClient::new(api_key))
            }
            _ => panic!("Only Anthropic provider supported"),
        };

        let mut tools = ToolRegistry::new();
        tools.register(Box::new(ReadFileTool));
        tools.register(Box::new(WriteFileTool));
        tools.register(Box::new(ListFilesTool));
        tools.register(Box::new(RunCommandTool));

        Self {
            client,
            tools,
            spec_path,
            max_iterations: max_iterations.or(config.rustagent.max_iterations),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut iteration = 0;

        loop {
            iteration += 1;

            if let Some(max) = self.max_iterations {
                if iteration > max {
                    warn!("Reached max iterations ({})", max);
                    return Ok(());
                }
            }

            info!("Ralph loop iteration {}", iteration);

            // Load spec
            let mut spec = Spec::load(std::path::Path::new(&self.spec_path))
                .context("Failed to load spec")?;

            // Find next pending task
            let next_task = match spec.find_next_task() {
                Some(task) => task.clone(),
                None => {
                    info!("No more pending tasks - Ralph loop complete!");
                    return Ok(());
                }
            };

            println!("\n=== Starting Task: {} ===", next_task.title);
            println!("{}\n", next_task.description);

            // Mark task as in progress
            if let Some(task) = spec.find_task_mut(&next_task.id) {
                task.status = TaskStatus::InProgress;
            }
            spec.save(std::path::Path::new(&self.spec_path))?;

            // Build fresh context
            let context = self.build_context(&spec, &next_task);

            // Execute task
            match self.execute_task(context).await {
                Ok(learnings) => {
                    info!("Task completed successfully");

                    // Mark task as complete
                    if let Some(task) = spec.find_task_mut(&next_task.id) {
                        task.status = TaskStatus::Complete;
                        task.completed_at = Some(chrono::Utc::now().to_rfc3339());
                    }

                    // Add learnings
                    for learning in learnings {
                        spec.add_learning(learning);
                    }

                    spec.save(std::path::Path::new(&self.spec_path))?;
                }
                Err(e) => {
                    warn!("Task blocked: {}", e);

                    // Mark task as blocked
                    if let Some(task) = spec.find_task_mut(&next_task.id) {
                        task.status = TaskStatus::Blocked;
                        task.blocked_reason = Some(e.to_string());
                    }

                    spec.add_learning(format!("Task {} blocked: {}", next_task.id, e));
                    spec.save(std::path::Path::new(&self.spec_path))?;

                    return Err(e);
                }
            }

            // Delay before next iteration
            tokio::time::sleep(tokio::time::Duration::from_millis(DEFAULT_ITERATION_DELAY_MS)).await;
        }
    }

    fn build_context(&self, spec: &Spec, task: &crate::spec::Task) -> Vec<Message> {
        let system_prompt = format!(
            r#"You are a Ralph, an autonomous coding agent working on a task.

Project: {}
Description: {}

Your current task:
ID: {}
Title: {}
Description: {}

Acceptance Criteria:
{}

Recent learnings:
{}

Use the available tools to complete this task. When the task is complete according to all acceptance criteria, respond with "TASK_COMPLETE" followed by any learnings.

If you get stuck and cannot proceed, respond with "TASK_BLOCKED: [reason]".
"#,
            spec.name,
            spec.description,
            task.id,
            task.title,
            task.description,
            task.acceptance_criteria.iter().map(|c| format!("- {}", c)).collect::<Vec<_>>().join("\n"),
            spec.learnings.last().map(|l| l.as_str()).unwrap_or("None yet")
        );

        vec![Message {
            role: Role::System,
            content: system_prompt,
        }]
    }

    async fn execute_task(&mut self, mut messages: Vec<Message>) -> Result<Vec<String>> {
        let mut learnings = Vec::new();
        let max_turns = 50;

        for turn in 0..max_turns {
            info!("Task execution turn {}/{}", turn + 1, max_turns);

            let tool_defs = self.tools.definitions();
            let response = self.client.chat(messages.clone(), &tool_defs).await?;

            match response.content {
                ResponseContent::Text(text) => {
                    println!("{}", text);

                    if text.contains("TASK_COMPLETE") {
                        // Extract learnings after TASK_COMPLETE
                        if let Some(learning_text) = text.split("TASK_COMPLETE").nth(1) {
                            let learning = learning_text.trim().to_string();
                            if !learning.is_empty() {
                                learnings.push(learning);
                            }
                        }
                        return Ok(learnings);
                    }

                    if text.contains("TASK_BLOCKED") {
                        let reason = text
                            .split("TASK_BLOCKED:")
                            .nth(1)
                            .unwrap_or("Unknown reason")
                            .trim();
                        anyhow::bail!("{}", reason);
                    }

                    messages.push(Message {
                        role: Role::Assistant,
                        content: text,
                    });
                }
                ResponseContent::ToolCalls(calls) => {
                    let mut results = Vec::new();

                    for call in calls {
                        println!("Executing tool: {}", call.name);

                        if let Some(tool) = self.tools.get(&call.name) {
                            match tool.execute(call.parameters).await {
                                Ok(result) => {
                                    results.push(format!("Tool {}: {}", call.name, result));
                                }
                                Err(e) => {
                                    results.push(format!("Tool {} error: {}", call.name, e));
                                }
                            }
                        }
                    }

                    let result_text = results.join("\n\n");
                    messages.push(Message {
                        role: Role::User,
                        content: format!("Tool results:\n{}", result_text),
                    });
                }
            }
        }

        anyhow::bail!("Exceeded maximum turns without completion")
    }
}
```

Add dependency to `Cargo.toml`:
```toml
chrono = "0.4"
```

**Step 5: Export ralph module**

Modify `src/main.rs` to add after planning module:
```rust
pub mod ralph;
```

**Step 6: Update run command to use Ralph loop**

In `src/main.rs`, replace the `Commands::Run` handler:

```rust
Commands::Run { spec_file, max_iterations } => {
    info!("Running Ralph loop on: {}", spec_file);

    // Load config
    let config_path = dirs::home_dir()
        .map(|h| h.join(".config/rustagent/config.toml"))
        .unwrap_or_else(|| "rustagent.toml".into());

    let config = if config_path.exists() {
        config::Config::load(&config_path)?
    } else {
        anyhow::bail!("Config file not found. Create ~/.config/rustagent/config.toml or rustagent.toml");
    };

    let mut ralph_loop = ralph::RalphLoop::new(config, spec_file, max_iterations);
    ralph_loop.run().await?;

    Ok(())
}
```

**Step 7: Run tests to verify they pass**

Run: `cargo test test_ralph`
Expected: PASS - both Ralph loop tests succeed

**Step 8: Commit**

```bash
git add src/ralph/ src/main.rs tests/ralph_test.rs Cargo.toml Cargo.lock
git commit -m "feat: implement Ralph loop with autonomous task execution"
```

---

## Phase 7: Polish and Documentation

### Task 11: Add Example Config and README

**Files:**
- Create: `rustagent.toml.example`
- Modify: `README.md`

**Step 1: Create example config**

Create `rustagent.toml.example`:

```toml
[llm]
provider = "anthropic"  # Options: "anthropic", "openai", "ollama"
model = "claude-sonnet-4-20250514"

# Anthropic configuration
[anthropic]
api_key = "${ANTHROPIC_API_KEY}"

# OpenAI configuration (optional)
# [openai]
# api_key = "${OPENAI_API_KEY}"

# Ollama configuration (optional)
# [ollama]
# base_url = "http://localhost:11434"

[rustagent]
spec_dir = "specs"        # Default spec directory
max_iterations = null     # null = unlimited, or set a number
```

**Step 2: Update README**

Modify `README.md`:

```markdown
# Rustagent

A Rust implementation of the Ralph Loop pattern for autonomous AI agent development.

## Overview

Rustagent is a CLI tool with two modes:
1. **Planning Agent** - Interactive brainstorming to create structured specs
2. **Ralph Loop** - Autonomous task execution with fresh context per iteration

## Installation

```bash
cargo install --path .
```

Or run directly:

```bash
cargo run -- <command>
```

## Configuration

Create a config file at `~/.config/rustagent/config.toml` or `./rustagent.toml`:

```toml
[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"

[anthropic]
api_key = "${ANTHROPIC_API_KEY}"

[rustagent]
spec_dir = "specs"
max_iterations = null  # unlimited
```

See `rustagent.toml.example` for a complete example.

## Usage

### Initialize Spec Directory

```bash
rustagent init --spec-dir specs
```

### Planning Mode

Interactive conversation to create a spec:

```bash
rustagent plan --spec-dir specs
```

The agent will ask clarifying questions and create a detailed spec JSON file.

### Run Ralph Loop

Execute tasks autonomously from a spec file:

```bash
rustagent run specs/my-feature.json
```

Optional flags:
- `--max-iterations <n>` - Limit iterations (default: unlimited)

## How It Works

### Planning Agent

1. Start interactive conversation
2. Ask clarifying questions one at a time
3. Research codebase using tools
4. Break work into discrete tasks
5. Write spec JSON to spec directory

### Ralph Loop

1. Load spec file
2. Find first pending task
3. Build fresh context with task details
4. Execute task with available tools
5. Mark complete or blocked
6. Save learnings to spec
7. Repeat until all tasks complete

Key properties:
- Fresh LLM context each iteration
- Persistence via spec file
- One task per iteration
- Immediate exit on blocked state

## Spec Format

Specs are JSON files with this structure:

```json
{
  "name": "feature-name",
  "description": "High-level description",
  "branch_name": "feature/feature-name",
  "created_at": "2026-01-19T12:00:00Z",
  "tasks": [
    {
      "id": "task-1",
      "title": "Implement X",
      "description": "Detailed description",
      "acceptance_criteria": ["Criterion 1"],
      "status": "pending",
      "blocked_reason": null,
      "completed_at": null
    }
  ],
  "learnings": []
}
```

Task statuses: `pending`, `in_progress`, `complete`, `blocked`

## Available Tools

Both planning and Ralph modes have access to:

- `read_file` - Read file contents
- `write_file` - Write/create a file
- `list_files` - List directory contents
- `run_command` - Execute shell command

## Development

### Build

```bash
cargo build
```

### Test

```bash
cargo test
```

### Run with Logging

```bash
cargo run -- -vvv plan
```

## References

- [Everything is a Ralph Loop](https://ghuntley.com/loop/)
- [Ralph Wiggum as a Software Engineer](https://ghuntley.com/ralph/)
- [snarktank/ralph](https://github.com/snarktank/ralph)

## License

MIT
```

**Step 3: Verify files are correct**

Run: `cat rustagent.toml.example`
Run: `cat README.md`
Expected: Both files display correctly

**Step 4: Commit**

```bash
git add rustagent.toml.example README.md
git commit -m "docs: add example config and comprehensive README"
```

---

## Final Steps

### Task 12: Integration Test

**Files:**
- Create: `tests/integration_test.rs`

**Step 1: Write end-to-end integration test**

Create `tests/integration_test.rs`:

```rust
use rustagent::config::{Config, LlmProvider, LlmConfig, AnthropicConfig, RustagentConfig};
use rustagent::spec::{Spec, Task, TaskStatus};
use tempfile::TempDir;
use std::fs;

#[test]
fn test_full_workflow() {
    let temp = TempDir::new().unwrap();

    // Create config
    let config = Config {
        llm: LlmConfig {
            provider: LlmProvider::Anthropic,
            model: "claude-sonnet-4-20250514".to_string(),
        },
        anthropic: Some(AnthropicConfig {
            api_key: "test-key".to_string(),
        }),
        openai: None,
        ollama: None,
        rustagent: RustagentConfig {
            spec_dir: "specs".to_string(),
            max_iterations: Some(10),
        },
    };

    // Create spec
    let spec = Spec {
        name: "test-feature".to_string(),
        description: "A test feature".to_string(),
        branch_name: "feature/test".to_string(),
        created_at: "2026-01-19T12:00:00Z".to_string(),
        tasks: vec![
            Task {
                id: "task-1".to_string(),
                title: "First task".to_string(),
                description: "Do the first thing".to_string(),
                acceptance_criteria: vec!["Must work".to_string()],
                status: TaskStatus::Pending,
                blocked_reason: None,
                completed_at: None,
            },
        ],
        learnings: vec![],
    };

    let spec_path = temp.path().join("test.json");
    spec.save(&spec_path).unwrap();

    // Verify spec was saved
    let loaded_spec = Spec::load(&spec_path).unwrap();
    assert_eq!(loaded_spec.name, "test-feature");
    assert_eq!(loaded_spec.tasks.len(), 1);

    // Verify config structure
    assert_eq!(config.llm.provider, LlmProvider::Anthropic);
    assert!(config.anthropic.is_some());
}
```

**Step 2: Run test to verify it passes**

Run: `cargo test test_full_workflow`
Expected: PASS - integration test succeeds

**Step 3: Run all tests**

Run: `cargo test`
Expected: All tests pass

**Step 4: Build release binary**

Run: `cargo build --release`
Expected: Binary builds successfully

**Step 5: Commit**

```bash
git add tests/integration_test.rs
git commit -m "test: add end-to-end integration test"
```

---

## Summary

This plan implements a complete Rustagent system with:

✅ Configuration system with TOML and env vars
✅ LLM client abstraction (Anthropic implemented)
✅ Modular tool system (file and shell tools)
✅ JSON spec persistence
✅ CLI with init, plan, and run commands
✅ Planning agent for interactive spec creation
✅ Ralph loop for autonomous task execution
✅ Comprehensive test coverage
✅ Documentation and examples

**Next steps after implementation:**
1. Add OpenAI and Ollama client implementations
2. Add web_search tool for planning agent
3. Add edit_file tool for more precise file modifications
4. Improve error handling and recovery
5. Add telemetry and logging improvements
