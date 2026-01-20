use async_trait::async_trait;
use anyhow::{Context, Result};
use serde_json::json;
use tokio::process::Command;

use crate::tools::Tool;

/// Tool for executing shell commands
pub struct RunCommandTool;

#[async_trait]
impl Tool for RunCommandTool {
    fn name(&self) -> &str {
        "run_command"
    }

    fn description(&self) -> &str {
        "Execute a shell command and return its output. Supports optional working directory."
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
                    "description": "Optional working directory for command execution"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> Result<String> {
        let command = params["command"]
            .as_str()
            .context("command parameter is required")?;

        let working_dir = params["working_dir"].as_str();

        // Use sh -c on Unix, cmd /C on Windows
        #[cfg(unix)]
        let (shell, shell_arg) = ("sh", "-c");

        #[cfg(windows)]
        let (shell, shell_arg) = ("cmd", "/C");

        let mut cmd = Command::new(shell);
        cmd.arg(shell_arg).arg(command);

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        let output = cmd
            .output()
            .await
            .context("Failed to execute command")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            if stderr.is_empty() {
                Ok(stdout.to_string())
            } else {
                Ok(format!("stdout:\n{}\n\nstderr:\n{}", stdout, stderr))
            }
        } else {
            let exit_code = output.status.code().unwrap_or(-1);
            Ok(format!(
                "Command failed with exit code {}:\nstdout:\n{}\nstderr:\n{}",
                exit_code, stdout, stderr
            ))
        }
    }
}
