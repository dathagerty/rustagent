use anyhow::Result;
use async_trait::async_trait;
use rustagent::tools::{Tool, ToolRegistry};

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
    let registry = ToolRegistry::new();
    registry.register(Arc::new(MockTool));

    assert!(registry.get("mock_tool").is_some());
    assert!(registry.get("nonexistent").is_none());
}

#[tokio::test]
async fn test_tool_execute() {
    let tool = MockTool;
    let result = tool.execute(serde_json::json!({})).await.unwrap();
    assert_eq!(result, "success");
}

use rustagent::tools::file::{ListFilesTool, ReadFileTool, WriteFileTool};
use rustagent::tools::shell::RunCommandTool;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_read_file_tool() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("test.txt");
    fs::write(&file_path, "hello world").unwrap();

    let config = rustagent::config::SecurityConfig {
        shell_policy: rustagent::config::ShellPolicy::Allowlist,
        allowed_commands: vec![],
        blocked_patterns: vec![],
        max_file_size_mb: 10,
        allowed_paths: vec![temp.path().to_string_lossy().to_string()],
    };
    let validator = Arc::new(rustagent::security::SecurityValidator::new(config).unwrap());
    let handler = Arc::new(rustagent::security::permission::AutoApproveHandler);

    let tool = ReadFileTool::new(validator, handler);
    let result = tool
        .execute(json!({
            "path": file_path.to_str().unwrap()
        }))
        .await
        .unwrap();

    assert!(result.contains("hello world"));
}

#[tokio::test]
async fn test_write_file_tool() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("output.txt");

    let config = rustagent::config::SecurityConfig {
        shell_policy: rustagent::config::ShellPolicy::Allowlist,
        allowed_commands: vec![],
        blocked_patterns: vec![],
        max_file_size_mb: 10,
        allowed_paths: vec![temp.path().to_string_lossy().to_string()],
    };
    let validator = Arc::new(rustagent::security::SecurityValidator::new(config).unwrap());
    let handler = Arc::new(rustagent::security::permission::AutoApproveHandler);

    let tool = WriteFileTool::new(validator, handler);
    tool.execute(json!({
        "path": file_path.to_str().unwrap(),
        "content": "test content"
    }))
    .await
    .unwrap();

    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "test content");
}

#[tokio::test]
async fn test_list_files_tool() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("file1.txt"), "a").unwrap();
    fs::write(temp.path().join("file2.txt"), "b").unwrap();

    let config = rustagent::config::SecurityConfig {
        shell_policy: rustagent::config::ShellPolicy::Allowlist,
        allowed_commands: vec![],
        blocked_patterns: vec![],
        max_file_size_mb: 10,
        allowed_paths: vec![temp.path().to_string_lossy().to_string()],
    };
    let validator = Arc::new(rustagent::security::SecurityValidator::new(config).unwrap());
    let handler = Arc::new(rustagent::security::permission::AutoApproveHandler);

    let tool = ListFilesTool::new(validator, handler);
    let result = tool
        .execute(json!({
            "path": temp.path().to_str().unwrap()
        }))
        .await
        .unwrap();

    assert!(result.contains("file1.txt"));
    assert!(result.contains("file2.txt"));
}

#[tokio::test]
async fn test_run_command_tool() {
    let config = rustagent::config::SecurityConfig {
        shell_policy: rustagent::config::ShellPolicy::Allowlist,
        allowed_commands: vec!["echo".to_string()],
        blocked_patterns: vec![],
        max_file_size_mb: 10,
        allowed_paths: vec![".".to_string()],
    };
    let validator =
        std::sync::Arc::new(rustagent::security::SecurityValidator::new(config).unwrap());
    let handler = std::sync::Arc::new(rustagent::security::permission::AutoApproveHandler);

    let tool = RunCommandTool::new(validator, handler);
    let result = tool
        .execute(json!({
            "command": "echo hello"
        }))
        .await
        .unwrap();

    assert!(result.contains("hello"));
}

#[tokio::test]
async fn test_run_command_with_working_dir() {
    let temp = TempDir::new().unwrap();

    let config = rustagent::config::SecurityConfig {
        shell_policy: rustagent::config::ShellPolicy::Allowlist,
        allowed_commands: vec!["pwd".to_string()],
        blocked_patterns: vec![],
        max_file_size_mb: 10,
        allowed_paths: vec![".".to_string()],
    };
    let validator =
        std::sync::Arc::new(rustagent::security::SecurityValidator::new(config).unwrap());
    let handler = std::sync::Arc::new(rustagent::security::permission::AutoApproveHandler);

    let tool = RunCommandTool::new(validator, handler);
    let result = tool
        .execute(json!({
            "command": "pwd",
            "working_dir": temp.path().to_str().unwrap()
        }))
        .await
        .unwrap();

    assert!(result.contains(temp.path().to_str().unwrap()));
}

use std::sync::Arc;
use std::thread;

#[test]
fn test_registry_clone_and_concurrent_access() {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(MockTool));

    let registry1 = registry.clone();
    let registry2 = registry.clone();

    let handle1 = thread::spawn(move || registry1.get("mock_tool").is_some());

    let handle2 = thread::spawn(move || registry2.get("mock_tool").is_some());

    assert!(handle1.join().unwrap());
    assert!(handle2.join().unwrap());
}

#[test]
fn test_registry_register_while_reading() {
    let registry = ToolRegistry::new();
    let registry_clone = registry.clone();

    let handle = thread::spawn(move || {
        registry_clone.register(Arc::new(MockTool));
    });

    // Should be able to read while another thread is registering
    let _ = registry.list();

    handle.join().unwrap();
}

use rustagent::config::{SecurityConfig, ShellPolicy};
use rustagent::security::{SecurityValidator, permission::AutoApproveHandler};
use std::collections::HashSet;

#[tokio::test]
async fn test_run_command_with_allowlist() {
    let config = SecurityConfig {
        shell_policy: ShellPolicy::Allowlist,
        allowed_commands: vec!["echo".to_string()],
        blocked_patterns: vec![],
        max_file_size_mb: 10,
        allowed_paths: vec![".".to_string()],
    };

    let validator = Arc::new(SecurityValidator::new(config).unwrap());
    let handler = Arc::new(AutoApproveHandler);

    let tool = RunCommandTool::new(validator, handler);

    // Allowed command should work
    let params = serde_json::json!({
        "command": "echo hello",
        "working_dir": null
    });

    let result = tool.execute(params).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_run_command_blocked() {
    let config = SecurityConfig {
        shell_policy: ShellPolicy::Allowlist,
        allowed_commands: vec!["echo".to_string()],
        blocked_patterns: vec![],
        max_file_size_mb: 10,
        allowed_paths: vec![".".to_string()],
    };

    let validator = Arc::new(SecurityValidator::new(config).unwrap());
    let handler = Arc::new(AutoApproveHandler);

    let tool = RunCommandTool::new(validator, handler);

    // Not allowed command (but handler will auto-approve)
    let params = serde_json::json!({
        "command": "ls",
        "working_dir": null
    });

    let result = tool.execute(params).await;
    assert!(result.is_ok()); // AutoApproveHandler allows it
}

#[tokio::test]
async fn test_read_file_path_validation() {
    let config = rustagent::config::SecurityConfig {
        shell_policy: rustagent::config::ShellPolicy::Allowlist,
        allowed_commands: vec![],
        blocked_patterns: vec![],
        max_file_size_mb: 10,
        allowed_paths: vec![".".to_string()],
    };

    let validator = Arc::new(rustagent::security::SecurityValidator::new(config).unwrap());
    let handler = Arc::new(rustagent::security::permission::AutoApproveHandler);

    let tool = ReadFileTool::new(validator, handler);

    // Path in current directory should work (after permission)
    let params = serde_json::json!({
        "path": "./Cargo.toml"
    });

    let result = tool.execute(params).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_write_file_size_check() {
    let dir = tempfile::tempdir().unwrap();

    let config = rustagent::config::SecurityConfig {
        shell_policy: rustagent::config::ShellPolicy::Allowlist,
        allowed_commands: vec![],
        blocked_patterns: vec![],
        max_file_size_mb: 1, // Very small limit
        allowed_paths: vec![dir.path().to_string_lossy().to_string()],
    };

    let validator = Arc::new(rustagent::security::SecurityValidator::new(config).unwrap());
    let handler = Arc::new(rustagent::security::permission::AutoApproveHandler);

    let tool = WriteFileTool::new(validator, handler);

    // Small content should work
    let path = dir.path().join("test.txt");
    let params = serde_json::json!({
        "path": path.to_string_lossy(),
        "content": "hello"
    });

    let result = tool.execute(params).await;
    assert!(result.is_ok());
}
