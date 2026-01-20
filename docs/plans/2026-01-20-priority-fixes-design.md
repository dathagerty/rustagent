# Priority Fixes Design - Rustagent

**Date:** 2026-01-20
**Status:** Design Complete
**Priority:** High & Medium fixes from initial implementation

## Overview

This design addresses all high and medium priority issues identified during the Rustagent initial implementation. Fixes are organized into three phases: Code Quality, Security, and Enhanced Features.

## Goals

1. **Code Quality**: Fix clippy warnings, improve type safety, thread safety
2. **Security**: Harden shell execution, protect file operations, prevent resource exhaustion
3. **Enhanced Features**: Per-mode LLM configuration, system message support, better error handling

## Phase 1: Code Quality & Quick Wins

### 1.1 Fix Clippy Warnings (4 total)

**File:** `src/planning/mod.rs:129`
- Issue: Collapsible if statement
- Fix: Combine nested if conditions
- Before: `if condition { if other { ... } }`
- After: `if condition && other { ... }`

**Files:** `src/ralph/mod.rs:257, 265, 275`
- Issue: Single-character string literals
- Fix: Use char literals instead
- Before: `"\n"`
- After: `'\n'`

### 1.2 Timestamp Type Migration

**Files:** `src/spec.rs`, `src/ralph/mod.rs`

**Change Spec and Task structs:**
```rust
use chrono::{DateTime, Utc};

pub struct Spec {
    pub name: String,
    pub description: String,
    pub branch_name: String,
    pub created_at: DateTime<Utc>,  // Changed from String
    pub tasks: Vec<Task>,
    pub learnings: Vec<String>,
}

pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub acceptance_criteria: Vec<String>,
    pub status: TaskStatus,
    pub blocked_reason: Option<String>,
    pub completed_at: Option<DateTime<Utc>>,  // Changed from String
}
```

**Impact:**
- Type safety for timestamps
- Easier date arithmetic
- Automatic RFC3339 serialization via serde
- Update RalphLoop to use `Utc::now()` directly instead of `.to_rfc3339()`

### 1.3 Thread-Safe Tool Registry

**File:** `src/tools/mod.rs`

**Current:**
```rust
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}
```

**New:**
```rust
use std::sync::{Arc, RwLock};

pub struct ToolRegistry {
    tools: Arc<RwLock<HashMap<String, Arc<dyn Tool>>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register(&self, tool: Arc<dyn Tool>) {
        let mut tools = self.tools.write().unwrap();
        tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let tools = self.tools.read().unwrap();
        tools.get(name).cloned()
    }
}

impl Clone for ToolRegistry {
    fn clone(&self) -> Self {
        Self {
            tools: Arc::clone(&self.tools),
        }
    }
}
```

**Benefits:**
- Safe concurrent access from multiple threads
- Tools can be shared via Arc
- Registry is cheaply cloneable (just clones the Arc)

### 1.4 Parent Directory Creation in Spec::save()

**File:** `src/spec.rs`

**Add to save() method:**
```rust
pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();

    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .context("Failed to create spec directory")?;
    }

    let json = serde_json::to_string_pretty(self)
        .context("Failed to serialize spec")?;

    fs::write(path, json)
        .context("Failed to write spec file")?;

    Ok(())
}
```

## Phase 2: Security Hardening

### 2.1 Security Configuration

**File:** `src/config.rs`

**Add SecurityConfig:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default = "default_shell_policy")]
    pub shell_policy: ShellPolicy,

    #[serde(default = "default_allowed_commands")]
    pub allowed_commands: Vec<String>,

    #[serde(default)]
    pub blocked_patterns: Vec<String>,

    #[serde(default = "default_max_file_size_mb")]
    pub max_file_size_mb: u64,

    #[serde(default = "default_allowed_paths")]
    pub allowed_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShellPolicy {
    Allowlist,
    Blocklist,
    Unrestricted,
}

fn default_shell_policy() -> ShellPolicy {
    ShellPolicy::Allowlist
}

fn default_allowed_commands() -> Vec<String> {
    vec![
        "git".to_string(),
        "cargo".to_string(),
        "npm".to_string(),
        "ls".to_string(),
        "cat".to_string(),
        "grep".to_string(),
        "find".to_string(),
        "echo".to_string(),
        "pwd".to_string(),
        "mkdir".to_string(),
        "touch".to_string(),
    ]
}

fn default_max_file_size_mb() -> u64 {
    10
}

fn default_allowed_paths() -> Vec<String> {
    vec![".".to_string()]
}

// Add to Config struct:
pub struct Config {
    pub llm: LlmConfig,
    pub anthropic: Option<AnthropicConfig>,
    pub openai: Option<OpenAiConfig>,
    pub ollama: Option<OllamaConfig>,
    pub rustagent: RustagentConfig,
    #[serde(default)]
    pub security: SecurityConfig,  // NEW
}
```

**Example config:**
```toml
[security]
shell_policy = "allowlist"
allowed_commands = ["git", "cargo", "npm", "ls", "cat", "grep"]
blocked_patterns = ["rm -rf /", "eval", "curl.*\\|.*sh"]
max_file_size_mb = 10
allowed_paths = [".", "~/.config/rustagent"]
```

### 2.2 Security Validation Module

**File:** `src/security/mod.rs`

```rust
use anyhow::{anyhow, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::fs;

pub struct SecurityValidator {
    config: SecurityConfig,
    blocked_regexes: Vec<Regex>,
    allowed_paths_canonical: Vec<PathBuf>,
}

impl SecurityValidator {
    pub fn new(config: SecurityConfig) -> Result<Self> {
        // Compile blocked patterns into regexes
        let blocked_regexes = config
            .blocked_patterns
            .iter()
            .map(|p| Regex::new(p))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow!("Invalid regex pattern: {}", e))?;

        // Canonicalize allowed paths
        let allowed_paths_canonical = config
            .allowed_paths
            .iter()
            .map(|p| {
                let expanded = shellexpand::tilde(p);
                PathBuf::from(expanded.as_ref())
                    .canonicalize()
                    .unwrap_or_else(|_| PathBuf::from(expanded.as_ref()))
            })
            .collect();

        Ok(Self {
            config,
            blocked_regexes,
            allowed_paths_canonical,
        })
    }

    pub fn validate_shell_command(&self, command: &str) -> ValidationResult {
        match self.config.shell_policy {
            ShellPolicy::Unrestricted => ValidationResult::Allowed,

            ShellPolicy::Allowlist => {
                // Extract base command (first word)
                let base_cmd = command.split_whitespace().next().unwrap_or("");

                if self.config.allowed_commands.contains(&base_cmd.to_string()) {
                    ValidationResult::Allowed
                } else {
                    ValidationResult::RequiresPermission(
                        format!("Command '{}' not in allowlist", base_cmd)
                    )
                }
            }

            ShellPolicy::Blocklist => {
                for pattern in &self.blocked_regexes {
                    if pattern.is_match(command) {
                        return ValidationResult::Denied(
                            format!("Command matches blocked pattern: {}", pattern)
                        );
                    }
                }
                ValidationResult::Allowed
            }
        }
    }

    pub fn validate_file_path(&self, path: &Path) -> ValidationResult {
        // Canonicalize the requested path
        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                // Path doesn't exist yet, try to canonicalize parent
                if let Some(parent) = path.parent() {
                    match parent.canonicalize() {
                        Ok(p) => p.join(path.file_name().unwrap()),
                        Err(_) => return ValidationResult::Denied(
                            "Cannot resolve path".to_string()
                        ),
                    }
                } else {
                    return ValidationResult::Denied(
                        "Invalid path".to_string()
                    );
                }
            }
        };

        // Check if path is within allowed paths
        for allowed in &self.allowed_paths_canonical {
            if canonical.starts_with(allowed) {
                return ValidationResult::Allowed;
            }
        }

        ValidationResult::RequiresPermission(
            format!("Path '{}' is outside allowed directories", path.display())
        )
    }

    pub fn check_file_size(&self, path: &Path) -> ValidationResult {
        match fs::metadata(path) {
            Ok(metadata) => {
                let size_mb = metadata.len() / (1024 * 1024);
                if size_mb <= self.config.max_file_size_mb {
                    ValidationResult::Allowed
                } else {
                    ValidationResult::Denied(
                        format!(
                            "File size {}MB exceeds limit of {}MB",
                            size_mb,
                            self.config.max_file_size_mb
                        )
                    )
                }
            }
            Err(e) => ValidationResult::Denied(
                format!("Cannot check file size: {}", e)
            ),
        }
    }
}

#[derive(Debug)]
pub enum ValidationResult {
    Allowed,
    Denied(String),
    RequiresPermission(String),
}
```

### 2.3 Permission Handler

**File:** `src/security/permission.rs`

```rust
use std::io::{self, Write};

pub trait PermissionHandler: Send + Sync {
    fn request_permission(&self, request: &PermissionRequest) -> PermissionResult;
}

pub struct PermissionRequest {
    pub resource_type: ResourceType,
    pub action: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy)]
pub enum ResourceType {
    ShellCommand,
    FilePath,
}

#[derive(Debug)]
pub enum PermissionResult {
    Allow,
    Deny,
    AllowAlways(String),  // Remember this decision for future
}

pub struct CliPermissionHandler;

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
                // Extract the resource to remember
                let resource = match request.resource_type {
                    ResourceType::ShellCommand => {
                        // Extract base command
                        request.action.split_whitespace().next().unwrap_or("")
                    }
                    ResourceType::FilePath => &request.action,
                };
                PermissionResult::AllowAlways(resource.to_string())
            }
            "q" | "quit" => std::process::exit(0),
            _ => PermissionResult::Deny,
        }
    }
}
```

### 2.4 Tool Integration

**Update RunCommandTool:**

```rust
pub struct RunCommandTool {
    validator: Arc<SecurityValidator>,
    permission_handler: Arc<dyn PermissionHandler>,
    runtime_allowed: Arc<RwLock<HashSet<String>>>,
}

impl RunCommandTool {
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

    async fn execute(&self, params: serde_json::Value) -> Result<String> {
        let params: RunCommandParams = serde_json::from_value(params)?;

        // Check if command was previously allowed
        let base_cmd = params.command.split_whitespace().next().unwrap_or("");
        {
            let allowed = self.runtime_allowed.read().unwrap();
            if allowed.contains(base_cmd) {
                return self.execute_command(&params).await;
            }
        }

        // Validate command
        match self.validator.validate_shell_command(&params.command) {
            ValidationResult::Allowed => {
                self.execute_command(&params).await
            }
            ValidationResult::Denied(reason) => {
                anyhow::bail!("Command denied: {}", reason)
            }
            ValidationResult::RequiresPermission(reason) => {
                let request = PermissionRequest {
                    resource_type: ResourceType::ShellCommand,
                    action: params.command.clone(),
                    reason,
                };

                match self.permission_handler.request_permission(&request) {
                    PermissionResult::Allow => {
                        self.execute_command(&params).await
                    }
                    PermissionResult::Deny => {
                        anyhow::bail!("Permission denied by user")
                    }
                    PermissionResult::AllowAlways(cmd) => {
                        let mut allowed = self.runtime_allowed.write().unwrap();
                        allowed.insert(cmd);
                        self.execute_command(&params).await
                    }
                }
            }
        }
    }

    async fn execute_command(&self, params: &RunCommandParams) -> Result<String> {
        // Existing command execution logic...
    }
}
```

**Update file tools similarly with path validation and file size checks.**

## Phase 3: Enhanced Features

### 3.1 Per-Mode LLM Configuration

**File:** `src/config.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub llm: LlmConfig,  // Default/fallback

    pub anthropic: Option<AnthropicConfig>,
    pub openai: Option<OpenAiConfig>,
    pub ollama: Option<OllamaConfig>,

    pub planning: Option<ModeConfig>,  // NEW
    pub ralph: Option<ModeConfig>,     // NEW

    pub rustagent: RustagentConfig,

    #[serde(default)]
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeConfig {
    pub llm: LlmConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

fn default_max_tokens() -> u32 {
    8192
}

impl Config {
    pub fn planning_llm(&self) -> &LlmConfig {
        self.planning
            .as_ref()
            .map(|m| &m.llm)
            .unwrap_or(&self.llm)
    }

    pub fn ralph_llm(&self) -> &LlmConfig {
        self.ralph
            .as_ref()
            .map(|m| &m.llm)
            .unwrap_or(&self.llm)
    }
}
```

**Example config:**
```toml
# Default LLM (fallback)
[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
max_tokens = 4096

# Planning mode uses Opus
[planning.llm]
provider = "anthropic"
model = "claude-opus-4-20241120"
max_tokens = 8192

# Ralph mode uses Sonnet
[ralph.llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
max_tokens = 4096

[anthropic]
api_key = "${ANTHROPIC_API_KEY}"
```

### 3.2 System Message Support

**File:** `src/llm/anthropic.rs`

```rust
#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: Option<String>,  // NEW
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
}

impl AnthropicClient {
    pub fn format_request(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<serde_json::Value> {
        // Extract system message
        let system_message = messages
            .iter()
            .find(|m| m.role == Role::System)
            .map(|m| m.content.clone());

        // Filter out system messages from messages array
        let anthropic_messages: Vec<AnthropicMessage> = messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(|m| AnthropicMessage {
                role: match m.role {
                    Role::User => "user".to_string(),
                    Role::Assistant => "assistant".to_string(),
                    Role::System => unreachable!(),
                },
                content: m.content.clone(),
            })
            .collect();

        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            system: system_message,  // NEW
            messages: anthropic_messages,
            tools: /* ... */,
        };

        Ok(serde_json::to_value(request)?)
    }
}
```

### 3.3 Enhanced Error Handling

**Replace panics with proper errors:**

**File:** `src/planning/mod.rs`
```rust
// OLD:
let client: Box<dyn LlmClient> = match config.llm.provider {
    LlmProvider::Anthropic => {
        let api_key = config
            .anthropic
            .expect("Anthropic config required")  // PANIC!
            .api_key;
        Box::new(AnthropicClient::new(api_key))
    }
    _ => panic!("Unsupported provider"),  // PANIC!
};

// NEW:
let llm_config = config.planning_llm();
let client: Box<dyn LlmClient> = match llm_config.provider {
    LlmProvider::Anthropic => {
        let anthropic_config = config.anthropic
            .as_ref()
            .ok_or_else(|| anyhow!("Anthropic provider selected but [anthropic] config missing"))?;

        Box::new(AnthropicClient::new(
            anthropic_config.api_key.clone(),
            llm_config.model.clone(),
            llm_config.max_tokens,
        ))
    }
    LlmProvider::OpenAi => {
        anyhow::bail!("OpenAI provider not yet implemented")
    }
    LlmProvider::Ollama => {
        anyhow::bail!("Ollama provider not yet implemented")
    }
};
```

**Add retry logic for API calls:**

**File:** `src/llm/anthropic.rs`
```rust
async fn chat(&self, messages: Vec<Message>, tools: &[ToolDefinition]) -> Result<Response> {
    let request_body = self.format_request(&messages, tools)?;

    let mut retries = 0;
    let max_retries = 3;

    loop {
        match self.send_request(&request_body).await {
            Ok(response) => return Ok(response),
            Err(e) if retries < max_retries && is_retryable(&e) => {
                retries += 1;
                let delay = Duration::from_secs(2u64.pow(retries));
                warn!("API call failed, retrying in {:?}: {}", delay, e);
                tokio::time::sleep(delay).await;
            }
            Err(e) => return Err(e),
        }
    }
}

fn is_retryable(error: &anyhow::Error) -> bool {
    // Check if error is retryable (network error, rate limit, etc.)
    error.to_string().contains("rate limit") ||
    error.to_string().contains("timeout") ||
    error.to_string().contains("connection")
}
```

## Implementation Order

1. **Phase 1: Code Quality** (30 minutes)
   - Fix clippy warnings
   - Migrate timestamps
   - Thread-safe registry
   - Parent directory creation

2. **Phase 2: Security** (2-3 hours)
   - SecurityConfig in config.rs
   - SecurityValidator module
   - PermissionHandler module
   - Integrate into tools
   - Add shellexpand dependency

3. **Phase 3: Enhanced Features** (1-2 hours)
   - Per-mode LLM config
   - Update AnthropicClient constructor
   - System message support
   - Replace panics with errors
   - Add retry logic

## Testing Strategy

1. **Unit tests** for SecurityValidator
2. **Integration tests** for permission flow
3. **Manual testing** of permission prompts
4. **Verify all existing tests still pass**
5. **Test with example configs**

## Dependencies to Add

```toml
regex = "1.10"
shellexpand = "3.1"
```

## Documentation Updates

1. Update `rustagent.toml.example` with security section
2. Update README.md with security features
3. Document permission system
4. Document per-mode LLM configuration

## Success Criteria

- ✅ All 4 clippy warnings fixed
- ✅ All timestamps use chrono types
- ✅ Tool registry is thread-safe
- ✅ Shell commands require permission by default
- ✅ File operations validate paths
- ✅ File size limits enforced
- ✅ Per-mode LLM configuration working
- ✅ System messages properly handled
- ✅ No panics in production code
- ✅ Basic retry logic for API failures
- ✅ All existing tests pass
- ✅ New security tests added
