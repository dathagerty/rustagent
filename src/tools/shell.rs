use crate::security::permission::{
    PermissionHandler, PermissionRequest, PermissionResult, ResourceType,
};
use crate::security::{SecurityValidator, ValidationResult};
use crate::tools::Tool;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::process::Stdio;
use std::sync::{Arc, RwLock};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

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

    async fn execute_command(&self, params: &RunCommandParams) -> Result<String> {
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", &params.command]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-c", &params.command]);
            c
        };

        if let Some(ref dir) = params.working_dir {
            cmd.current_dir(dir);
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()?;

        let mut stdout = String::new();
        let mut stderr = String::new();

        if let Some(mut out) = child.stdout.take() {
            out.read_to_string(&mut stdout).await?;
        }

        if let Some(mut err) = child.stderr.take() {
            err.read_to_string(&mut stderr).await?;
        }

        let status = child.wait().await?;

        let output = if !stdout.is_empty() { stdout } else { stderr };

        if status.success() {
            Ok(output)
        } else {
            anyhow::bail!("Command failed: {}", output)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct RunCommandParams {
    command: String,
    #[serde(default)]
    working_dir: Option<String>,
}

#[async_trait]
impl Tool for RunCommandTool {
    fn name(&self) -> &str {
        "run_command"
    }

    fn description(&self) -> &str {
        "Execute a shell command and return its output"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
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
        let params: RunCommandParams = serde_json::from_value(params)?;

        // Check if command was previously allowed
        let base_cmd = params.command.split_whitespace().next().unwrap_or("");
        let is_allowed = {
            let allowed = self.runtime_allowed.read().unwrap();
            allowed.contains(base_cmd)
        };
        if is_allowed {
            return self.execute_command(&params).await;
        }

        // Validate command
        match self.validator.validate_shell_command(&params.command) {
            ValidationResult::Allowed => self.execute_command(&params).await,
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
                    PermissionResult::Allow => self.execute_command(&params).await,
                    PermissionResult::Deny => {
                        anyhow::bail!("Permission denied by user")
                    }
                    PermissionResult::AllowAlways(cmd) => {
                        {
                            let mut allowed = self.runtime_allowed.write().unwrap();
                            allowed.insert(cmd);
                        }
                        self.execute_command(&params).await
                    }
                    PermissionResult::Quit => {
                        anyhow::bail!("User requested quit")
                    }
                }
            }
        }
    }
}
