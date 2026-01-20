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

use rustagent::tools::file::{ReadFileTool, WriteFileTool, ListFilesTool};
use rustagent::tools::shell::RunCommandTool;
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
