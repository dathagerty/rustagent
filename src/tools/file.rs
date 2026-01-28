use crate::security::permission::PermissionHandler;
use crate::security::{SecurityValidator, ValidationResult};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;

use super::Tool;
use super::permission_check::FilePermissionChecker;

/// Tool for reading file contents
pub struct ReadFileTool {
    checker: FilePermissionChecker,
    validator: Arc<SecurityValidator>,
}

impl ReadFileTool {
    pub fn new(
        validator: Arc<SecurityValidator>,
        permission_handler: Arc<dyn PermissionHandler>,
    ) -> Self {
        Self {
            checker: FilePermissionChecker::new(validator.clone(), permission_handler),
            validator,
        }
    }
}

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
        let path = Path::new(path);

        // Check permission
        self.checker.check_permission(path)?;

        // Check file size
        match self.validator.check_file_size(path) {
            ValidationResult::Allowed => {}
            ValidationResult::Denied(reason) => {
                anyhow::bail!("File too large: {}", reason);
            }
            ValidationResult::RequiresPermission(_) => {
                // Shouldn't happen for size checks
            }
        }

        let content = fs::read_to_string(path)
            .await
            .context(format!("Failed to read file: {}", path.display()))?;

        Ok(content)
    }
}

/// Tool for writing content to a file
pub struct WriteFileTool {
    checker: FilePermissionChecker,
    validator: Arc<SecurityValidator>,
}

impl WriteFileTool {
    pub fn new(
        validator: Arc<SecurityValidator>,
        permission_handler: Arc<dyn PermissionHandler>,
    ) -> Self {
        Self {
            checker: FilePermissionChecker::new(validator.clone(), permission_handler),
            validator,
        }
    }
}

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
        let path = Path::new(path);

        // Check permission
        self.checker.check_permission(path)?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.context(format!(
                "Failed to create parent directories for: {}",
                path.display()
            ))?;

            let canonical_parent = parent.canonicalize().context(format!(
                "Failed to resolve parent directory: {}",
                parent.display()
            ))?;
            match self.validator.validate_file_path(&canonical_parent) {
                ValidationResult::Allowed => {}
                _ => anyhow::bail!(
                    "Parent directory resolved outside allowed paths: {}",
                    canonical_parent.display()
                ),
            }
        }

        fs::write(path, content)
            .await
            .context(format!("Failed to write file: {}", path.display()))?;

        Ok(format!(
            "Successfully wrote {} bytes to {}",
            content.len(),
            path.display()
        ))
    }
}

/// Tool for listing files in a directory
pub struct ListFilesTool {
    checker: FilePermissionChecker,
}

impl ListFilesTool {
    pub fn new(
        validator: Arc<SecurityValidator>,
        permission_handler: Arc<dyn PermissionHandler>,
    ) -> Self {
        Self {
            checker: FilePermissionChecker::new(validator, permission_handler),
        }
    }
}

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
        let path = Path::new(path);

        // Check permission
        self.checker.check_permission(path)?;

        let mut entries = fs::read_dir(path)
            .await
            .context(format!("Failed to read directory: {}", path.display()))?;

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
