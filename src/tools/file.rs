use crate::security::permission::{
    PermissionHandler, PermissionRequest, PermissionResult, ResourceType,
};
use crate::security::{SecurityValidator, ValidationResult};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::fs;

use super::Tool;

/// Tool for reading file contents
pub struct ReadFileTool {
    validator: Arc<SecurityValidator>,
    permission_handler: Arc<dyn PermissionHandler>,
    runtime_allowed: Arc<RwLock<HashSet<String>>>,
}

impl ReadFileTool {
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

    fn check_permission(&self, path: &Path) -> Result<()> {
        let path_str = path.to_string_lossy().to_string();

        // Check runtime allowed
        let is_allowed = {
            let allowed = self.runtime_allowed.read().unwrap();
            allowed.contains(&path_str)
        };
        if is_allowed {
            return Ok(());
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
                        let mut allowed = self.runtime_allowed.write().unwrap();
                        allowed.insert(p);
                        Ok(())
                    }
                }
            }
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
        self.check_permission(path)?;

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
    validator: Arc<SecurityValidator>,
    permission_handler: Arc<dyn PermissionHandler>,
    runtime_allowed: Arc<RwLock<HashSet<String>>>,
}

impl WriteFileTool {
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

    fn check_permission(&self, path: &Path) -> Result<()> {
        let path_str = path.to_string_lossy().to_string();

        // Check runtime allowed
        let is_allowed = {
            let allowed = self.runtime_allowed.read().unwrap();
            allowed.contains(&path_str)
        };
        if is_allowed {
            return Ok(());
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
                        let mut allowed = self.runtime_allowed.write().unwrap();
                        allowed.insert(p);
                        Ok(())
                    }
                }
            }
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
        self.check_permission(path)?;

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.context(format!(
                "Failed to create parent directories for: {}",
                path.display()
            ))?;
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
    validator: Arc<SecurityValidator>,
    permission_handler: Arc<dyn PermissionHandler>,
    runtime_allowed: Arc<RwLock<HashSet<String>>>,
}

impl ListFilesTool {
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

    fn check_permission(&self, path: &Path) -> Result<()> {
        let path_str = path.to_string_lossy().to_string();

        // Check runtime allowed
        let is_allowed = {
            let allowed = self.runtime_allowed.read().unwrap();
            allowed.contains(&path_str)
        };
        if is_allowed {
            return Ok(());
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
                        let mut allowed = self.runtime_allowed.write().unwrap();
                        allowed.insert(p);
                        Ok(())
                    }
                }
            }
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
        self.check_permission(path)?;

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
