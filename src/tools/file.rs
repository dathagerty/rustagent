use async_trait::async_trait;
use anyhow::{Context, Result};
use serde_json::Value;
use tokio::fs;

use super::Tool;

/// Tool for reading file contents
pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Reads the contents of a file at the specified path"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to read"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let path = params["path"]
            .as_str()
            .context("Missing or invalid 'path' parameter")?;

        let content = fs::read_to_string(path)
            .await
            .context(format!("Failed to read file: {}", path))?;

        Ok(content)
    }
}

/// Tool for writing content to a file
pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Writes content to a file at the specified path, creating parent directories if needed"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let path = params["path"]
            .as_str()
            .context("Missing or invalid 'path' parameter")?;
        let content = params["content"]
            .as_str()
            .context("Missing or invalid 'content' parameter")?;

        // Create parent directories if they don't exist
        if let Some(parent) = std::path::Path::new(path).parent() {
            fs::create_dir_all(parent)
                .await
                .context(format!("Failed to create parent directories for: {}", path))?;
        }

        fs::write(path, content)
            .await
            .context(format!("Failed to write file: {}", path))?;

        Ok(format!("Successfully wrote {} bytes to {}", content.len(), path))
    }
}

/// Tool for listing files in a directory
pub struct ListFilesTool;

#[async_trait]
impl Tool for ListFilesTool {
    fn name(&self) -> &str {
        "list_files"
    }

    fn description(&self) -> &str {
        "Lists files and directories at the specified path"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the directory to list"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let path = params["path"]
            .as_str()
            .context("Missing or invalid 'path' parameter")?;

        let mut entries = fs::read_dir(path)
            .await
            .context(format!("Failed to read directory: {}", path))?;

        let mut items = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let metadata = entry.metadata().await?;
            let name = entry.file_name().to_string_lossy().to_string();

            if metadata.is_dir() {
                items.push(format!("{}/", name));
            } else {
                items.push(name);
            }
        }

        items.sort();
        Ok(items.join("\n"))
    }
}
