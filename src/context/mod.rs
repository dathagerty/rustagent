pub mod agents_md;

use crate::agent::AgentContext;
use crate::tools::Tool;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;

pub use agents_md::resolve_agents_md;

/// Builds a compact structured system prompt from an AgentContext
pub struct ContextBuilder;

impl ContextBuilder {
    /// Build a system prompt string from the given agent context
    pub fn build_system_prompt(ctx: &AgentContext) -> String {
        let mut prompt = String::new();

        // Role section
        prompt.push_str("## Role\n");
        prompt.push_str(&ctx.profile.role);
        prompt.push('\n');
        prompt.push('\n');

        // Task section - show work package tasks
        if !ctx.work_package_tasks.is_empty() {
            prompt.push_str("## Task\n");
            for task in &ctx.work_package_tasks {
                prompt.push_str(&format!(
                    "[TASK] {} | {} | priority={}\n",
                    task.id,
                    task.title,
                    task.priority
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "medium".to_string())
                ));

                // Add acceptance criteria if present in metadata
                if let Some(criteria) = task.metadata.get("acceptance_criteria") {
                    prompt.push_str(&format!("[CRITERIA] {}\n", criteria));
                }
            }
            prompt.push('\n');
        }

        // Session continuity - handoff notes
        if let Some(handoff) = &ctx.handoff_notes {
            prompt.push_str("## Session Continuity\n");
            prompt.push_str(&format!("[HANDOFF] {}\n", handoff));
            prompt.push('\n');
        }

        // Active decisions section
        if !ctx.relevant_decisions.is_empty() {
            prompt.push_str("## Active Decisions\n");
            for decision in &ctx.relevant_decisions {
                prompt.push_str(&format!(
                    "[DECISION] {} | {} | status={}\n",
                    decision.id, decision.title, decision.status
                ));

                // Add chosen option if present
                if let Some(chosen) = decision.metadata.get("chosen_option") {
                    prompt.push_str(&format!("  chosen: {}\n", chosen));
                }
            }
            prompt.push('\n');
        }

        // Relevant observations
        if !ctx.work_package_tasks.is_empty() {
            prompt.push_str("## Relevant Observations (use query_nodes(id) for full detail)\n");
            for task in &ctx.work_package_tasks {
                prompt.push_str(&format!("- {}: {}\n", task.id, task.description));
            }
            prompt.push('\n');
        }

        // Project conventions
        if !ctx.agents_md_summaries.is_empty() {
            prompt.push_str("## Project Conventions (use read_agents_md(path) for full text)\n");
            for (path, heading_summary) in &ctx.agents_md_summaries {
                prompt.push_str(&format!("- {}: {}\n", path, heading_summary));
            }
            prompt.push('\n');
        }

        // Rules from the profile
        prompt.push_str("## Rules\n");
        prompt.push_str(&ctx.profile.system_prompt);
        prompt.push('\n');

        prompt
    }
}

/// Tool for reading AGENTS.md files
pub struct ReadAgentsMdTool;

impl ReadAgentsMdTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ReadAgentsMdTool {
    fn name(&self) -> &str {
        "read_agents_md"
    }

    fn description(&self) -> &str {
        "Read the full contents of an AGENTS.md file to see detailed project conventions and guidelines"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the AGENTS.md file to read"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> Result<String> {
        let path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing 'path' parameter"))?;

        let path_buf = PathBuf::from(path);

        // Validate that the file is named AGENTS.md
        if path_buf.file_name() != Some(std::ffi::OsStr::new("AGENTS.md")) {
            return Err(anyhow::anyhow!(
                "read_agents_md can only read AGENTS.md files"
            ));
        }

        let content = std::fs::read_to_string(&path_buf)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {}", path, e))?;

        Ok(content)
    }
}

impl Default for ReadAgentsMdTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_agents_md_tool_name() {
        let tool = ReadAgentsMdTool::new();
        assert_eq!(tool.name(), "read_agents_md");
    }

    #[test]
    fn test_read_agents_md_tool_description() {
        let tool = ReadAgentsMdTool::new();
        let desc = tool.description();
        assert!(!desc.is_empty());
        assert!(desc.contains("AGENTS.md"));
    }

    #[test]
    fn test_read_agents_md_tool_parameters() {
        let tool = ReadAgentsMdTool::new();
        let params = tool.parameters();
        assert!(params.is_object());
        assert!(params["properties"]["path"].is_object());
        assert_eq!(params["required"][0], "path");
    }

    #[tokio::test]
    async fn test_read_agents_md_tool_execute() -> Result<()> {
        let tmpdir = tempfile::TempDir::new()?;
        let agents_md = tmpdir.path().join("AGENTS.md");
        std::fs::write(&agents_md, "# Test Guidelines\n\nContent here")?;

        let tool = ReadAgentsMdTool::new();
        let result = tool
            .execute(json!({
                "path": agents_md.to_string_lossy().to_string()
            }))
            .await?;

        assert!(result.contains("Test Guidelines"));
        assert!(result.contains("Content here"));
        Ok(())
    }

    #[tokio::test]
    async fn test_read_agents_md_tool_missing_file() {
        let tool = ReadAgentsMdTool::new();
        let result = tool
            .execute(json!({
                "path": "/nonexistent/AGENTS.md"
            }))
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_agents_md_tool_missing_path_param() {
        let tool = ReadAgentsMdTool::new();
        let result = tool.execute(json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_agents_md_tool_invalid_filename() {
        let tool = ReadAgentsMdTool::new();
        let result = tool
            .execute(json!({
                "path": "/some/path/README.md"
            }))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("AGENTS.md"));
    }

    #[tokio::test]
    async fn test_read_agents_md_tool_restricts_to_agents_md() {
        let tool = ReadAgentsMdTool::new();

        // Try to read a different file
        let result = tool
            .execute(json!({
                "path": "/etc/passwd"
            }))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("AGENTS.md"));
    }
}
