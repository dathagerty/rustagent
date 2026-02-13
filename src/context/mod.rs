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
        "Read the full contents of an AGENTS.md file to see detailed project conventions and guidelines. Accepts either a directory path (e.g., 'src/auth') or a full file path (e.g., 'src/auth/AGENTS.md')"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the AGENTS.md file or a directory containing AGENTS.md"
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

        // Check if path ends with AGENTS.md (direct file path)
        let final_path = if path.ends_with("AGENTS.md") {
            // Validate that the file is named AGENTS.md
            if path_buf.file_name() != Some(std::ffi::OsStr::new("AGENTS.md")) {
                return Err(anyhow::anyhow!(
                    "read_agents_md can only read AGENTS.md files"
                ));
            }
            path_buf
        } else if path_buf.extension().is_some() {
            // If path has a file extension but is not AGENTS.md, reject it explicitly
            return Err(anyhow::anyhow!(
                "read_agents_md can only read AGENTS.md files"
            ));
        } else {
            // Treat as directory and append /AGENTS.md
            path_buf.join("AGENTS.md")
        };

        let content = std::fs::read_to_string(&final_path)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {}", final_path.display(), e))?;

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
    async fn test_read_agents_md_tool_with_directory_path() -> Result<()> {
        let tmpdir = tempfile::TempDir::new()?;
        // Create AGENTS.md in the directory
        std::fs::write(
            tmpdir.path().join("AGENTS.md"),
            "# Test Guidelines\n\nContent",
        )?;

        let tool = ReadAgentsMdTool::new();
        let result = tool
            .execute(json!({
                "path": tmpdir.path().to_string_lossy().to_string()
            }))
            .await?;

        assert!(result.contains("Test Guidelines"));
        assert!(result.contains("Content"));
        Ok(())
    }

    #[tokio::test]
    async fn test_read_agents_md_tool_with_full_file_path() -> Result<()> {
        let tmpdir = tempfile::TempDir::new()?;
        let agents_md = tmpdir.path().join("AGENTS.md");
        std::fs::write(&agents_md, "# Test Guidelines\n\nContent")?;

        let tool = ReadAgentsMdTool::new();
        let result = tool
            .execute(json!({
                "path": agents_md.to_string_lossy().to_string()
            }))
            .await?;

        assert!(result.contains("Test Guidelines"));
        assert!(result.contains("Content"));
        Ok(())
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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("read_agents_md can only read AGENTS.md files")
        );
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

    #[test]
    fn test_build_system_prompt_output_format() {
        use crate::agent::profile::{AgentProfile, ProfileLlmConfig};
        use crate::graph::store::GraphStore;
        use crate::graph::{GraphNode, NodeStatus, NodeType, Priority};
        use crate::security::SecurityScope;
        use anyhow::Result;
        use async_trait::async_trait;
        use chrono::Utc;
        use std::collections::HashMap;
        use std::sync::Arc;

        // Minimal mock GraphStore for testing
        struct TestGraphStore;

        #[async_trait]
        impl GraphStore for TestGraphStore {
            async fn create_node(&self, _node: &GraphNode) -> Result<()> {
                Ok(())
            }
            async fn update_node(
                &self,
                _id: &str,
                _status: Option<NodeStatus>,
                _title: Option<&str>,
                _description: Option<&str>,
                _blocked_reason: Option<&str>,
                _metadata: Option<&HashMap<String, String>>,
            ) -> Result<()> {
                Ok(())
            }
            async fn get_node(&self, _id: &str) -> Result<Option<GraphNode>> {
                Ok(None)
            }
            async fn query_nodes(
                &self,
                _query: &crate::graph::store::NodeQuery,
            ) -> Result<Vec<GraphNode>> {
                Ok(vec![])
            }
            async fn claim_task(&self, _node_id: &str, _agent_id: &str) -> Result<bool> {
                Ok(false)
            }
            async fn get_ready_tasks(&self, _goal_id: &str) -> Result<Vec<GraphNode>> {
                Ok(vec![])
            }
            async fn get_next_task(&self, _goal_id: &str) -> Result<Option<GraphNode>> {
                Ok(None)
            }
            async fn add_edge(&self, _edge: &crate::graph::GraphEdge) -> Result<()> {
                Ok(())
            }
            async fn remove_edge(&self, _edge_id: &str) -> Result<()> {
                Ok(())
            }
            async fn get_edges(
                &self,
                _node_id: &str,
                _direction: crate::graph::store::EdgeDirection,
            ) -> Result<Vec<(crate::graph::GraphEdge, GraphNode)>> {
                Ok(vec![])
            }
            async fn get_children(
                &self,
                _node_id: &str,
            ) -> Result<Vec<(GraphNode, crate::graph::EdgeType)>> {
                Ok(vec![])
            }
            async fn get_subtree(&self, _node_id: &str) -> Result<Vec<GraphNode>> {
                Ok(vec![])
            }
            async fn get_active_decisions(&self, _project_id: &str) -> Result<Vec<GraphNode>> {
                Ok(vec![])
            }
            async fn get_full_graph(
                &self,
                _goal_id: &str,
            ) -> Result<crate::graph::store::WorkGraph> {
                Ok(crate::graph::store::WorkGraph {
                    nodes: vec![],
                    edges: vec![],
                })
            }
            async fn search_nodes(
                &self,
                _query: &str,
                _project_id: Option<&str>,
                _node_type: Option<NodeType>,
                _limit: usize,
            ) -> Result<Vec<GraphNode>> {
                Ok(vec![])
            }
            async fn next_child_seq(&self, _parent_id: &str) -> Result<u32> {
                Ok(1)
            }
            async fn import_nodes_and_edges(
                &self,
                _nodes: Vec<GraphNode>,
                _edges: Vec<crate::graph::GraphEdge>,
            ) -> Result<()> {
                Ok(())
            }
        }

        // Create mock profile
        let profile = AgentProfile {
            name: "test_coder".to_string(),
            extends: None,
            role: "You are a helpful code assistant".to_string(),
            system_prompt: "Follow these rules carefully".to_string(),
            allowed_tools: vec!["read_file".to_string(), "write_file".to_string()],
            security: SecurityScope {
                allowed_paths: vec!["*".to_string()],
                denied_paths: vec![],
                allowed_commands: vec!["*".to_string()],
                read_only: false,
                can_create_files: true,
                network_access: false,
            },
            llm: ProfileLlmConfig::default(),
            turn_limit: Some(100),
            token_budget: Some(100_000),
        };

        // Create mock work package tasks
        let mut task_metadata = HashMap::new();
        task_metadata.insert(
            "acceptance_criteria".to_string(),
            "AC1: Task should pass tests".to_string(),
        );

        let work_package_tasks = vec![GraphNode {
            id: "task-1".to_string(),
            project_id: "proj-1".to_string(),
            node_type: NodeType::Task,
            title: "Implement feature".to_string(),
            description: "Implement a new feature".to_string(),
            status: NodeStatus::Ready,
            priority: Some(Priority::High),
            assigned_to: None,
            created_by: None,
            labels: vec![],
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            blocked_reason: None,
            metadata: task_metadata,
        }];

        // Create mock decisions
        let mut decision_metadata = HashMap::new();
        decision_metadata.insert("chosen_option".to_string(), "Option B".to_string());

        let relevant_decisions = vec![GraphNode {
            id: "decision-1".to_string(),
            project_id: "proj-1".to_string(),
            node_type: NodeType::Decision,
            title: "Architecture decision".to_string(),
            description: "Choose architecture".to_string(),
            status: NodeStatus::Decided,
            priority: None,
            assigned_to: None,
            created_by: None,
            labels: vec![],
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            blocked_reason: None,
            metadata: decision_metadata,
        }];

        // Create agent context
        let ctx = AgentContext {
            work_package_tasks,
            relevant_decisions,
            handoff_notes: Some("Previous session notes".to_string()),
            agents_md_summaries: vec![("src/AGENTS.md".to_string(), "Code standards".to_string())],
            profile,
            project_path: PathBuf::from("/test/project"),
            graph_store: Arc::new(TestGraphStore),
        };

        // Build system prompt
        let prompt = ContextBuilder::build_system_prompt(&ctx);

        // Verify expected sections are present
        assert!(prompt.contains("## Role"), "Should contain Role section");
        assert!(
            prompt.contains("You are a helpful code assistant"),
            "Should contain profile role"
        );

        assert!(prompt.contains("## Task"), "Should contain Task section");
        assert!(prompt.contains("[TASK]"), "Should contain task marker");
        assert!(prompt.contains("task-1"), "Should contain task ID");
        assert!(
            prompt.contains("[CRITERIA]"),
            "Should contain acceptance criteria marker"
        );

        assert!(
            prompt.contains("## Session Continuity"),
            "Should contain Session Continuity section"
        );
        assert!(
            prompt.contains("[HANDOFF]"),
            "Should contain handoff marker"
        );
        assert!(
            prompt.contains("Previous session notes"),
            "Should contain handoff notes"
        );

        assert!(
            prompt.contains("## Active Decisions"),
            "Should contain Active Decisions section"
        );
        assert!(
            prompt.contains("[DECISION]"),
            "Should contain decision marker"
        );
        assert!(prompt.contains("decision-1"), "Should contain decision ID");
        assert!(prompt.contains("chosen:"), "Should contain chosen option");

        assert!(
            prompt.contains("## Relevant Observations"),
            "Should contain Relevant Observations section"
        );

        assert!(
            prompt.contains("## Project Conventions"),
            "Should contain Project Conventions section"
        );
        assert!(
            prompt.contains("src/AGENTS.md"),
            "Should contain agents_md path"
        );

        assert!(prompt.contains("## Rules"), "Should contain Rules section");
        assert!(
            prompt.contains("Follow these rules carefully"),
            "Should contain system prompt rules"
        );
    }
}
