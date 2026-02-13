pub mod agents_md;

use crate::agent::AgentContext;
use crate::tools::Tool;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;

pub use agents_md::resolve_agents_md;

/// Token budget for context assembly
#[derive(Clone, Debug)]
pub struct ContextBudget {
    /// Total token budget for the system prompt (default: 4000)
    pub max_tokens: usize,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self { max_tokens: 4000 }
    }
}

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

        // Dependency status section
        if !ctx.dependency_statuses.is_empty() {
            for (id, title, is_done) in &ctx.dependency_statuses {
                if *is_done {
                    prompt.push_str(&format!("[DEP:DONE] {} → {} (completed)\n", id, title));
                } else {
                    prompt.push_str(&format!("[DEP:PENDING] {} → {} (pending)\n", id, title));
                }
            }
        }

        // Previous attempt section
        if let Some(prev) = &ctx.previous_attempt {
            prompt.push_str("\n## Previous Attempt\n");
            prompt.push_str(&format!("[PREV_ATTEMPT] {}\n\n", prev));
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

    /// Build a system prompt with token budget awareness
    ///
    /// Prioritizes required sections (Role, Task, Dependencies, Previous Attempt, Rules) and trims optional sections
    /// (Session Continuity, Active Decisions, Observations, Project Conventions) when over budget.
    pub fn build_system_prompt_with_budget(ctx: &AgentContext, budget: &ContextBudget) -> String {
        // Token estimate: ~1 token per 4 characters
        let estimate_tokens = |text: &str| text.len() / 4;

        // Build required sections first (these are never trimmed)
        let mut required = String::new();

        // Role section
        required.push_str("## Role\n");
        required.push_str(&ctx.profile.role);
        required.push('\n');
        required.push('\n');

        // Task section - show work package tasks
        if !ctx.work_package_tasks.is_empty() {
            required.push_str("## Task\n");
            for task in &ctx.work_package_tasks {
                required.push_str(&format!(
                    "[TASK] {} | {} | priority={}\n",
                    task.id,
                    task.title,
                    task.priority
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "medium".to_string())
                ));

                // Add acceptance criteria if present in metadata
                if let Some(criteria) = task.metadata.get("acceptance_criteria") {
                    required.push_str(&format!("[CRITERIA] {}\n", criteria));
                }
            }
            required.push('\n');
        }

        // Dependency status section (required - agents need to know dependency status)
        if !ctx.dependency_statuses.is_empty() {
            for (id, title, is_done) in &ctx.dependency_statuses {
                if *is_done {
                    required.push_str(&format!("[DEP:DONE] {} → {} (completed)\n", id, title));
                } else {
                    required.push_str(&format!("[DEP:PENDING] {} → {} (pending)\n", id, title));
                }
            }
        }

        // Previous attempt section (required - agents retrying need this context)
        if let Some(prev) = &ctx.previous_attempt {
            required.push_str("\n## Previous Attempt\n");
            required.push_str(&format!("[PREV_ATTEMPT] {}\n\n", prev));
        }

        // Rules from the profile (required - critical for agent behavior)
        required.push_str("## Rules\n");
        required.push_str(&ctx.profile.system_prompt);
        required.push('\n');

        let required_tokens = estimate_tokens(&required);

        // If required sections already exceed budget, just return them
        if required_tokens >= budget.max_tokens {
            return required;
        }

        // Budget remaining for optional sections
        let mut remaining_budget = budget.max_tokens - required_tokens;
        let mut prompt = required;

        // Build optional sections in priority order
        // Priority 1 (highest): Session Continuity
        let mut session_section = String::new();
        if let Some(handoff) = &ctx.handoff_notes {
            session_section.push_str("## Session Continuity\n");
            session_section.push_str(&format!("[HANDOFF] {}\n", handoff));
            session_section.push('\n');
        }

        // Priority 2: Active Decisions
        let mut decisions_section = String::new();
        if !ctx.relevant_decisions.is_empty() {
            decisions_section.push_str("## Active Decisions\n");
            for decision in &ctx.relevant_decisions {
                decisions_section.push_str(&format!(
                    "[DECISION] {} | {} | status={}\n",
                    decision.id, decision.title, decision.status
                ));

                // Add chosen option if present
                if let Some(chosen) = decision.metadata.get("chosen_option") {
                    decisions_section.push_str(&format!("  chosen: {}\n", chosen));
                }
            }
            decisions_section.push('\n');
        }

        // Priority 3: Relevant Observations
        let mut observations_section = String::new();
        if !ctx.work_package_tasks.is_empty() {
            observations_section.push_str("## Relevant Observations (use query_nodes(id) for full detail)\n");
            for task in &ctx.work_package_tasks {
                observations_section.push_str(&format!("- {}: {}\n", task.id, task.description));
            }
            observations_section.push('\n');
        }

        // Priority 4 (lowest): Project Conventions
        let mut conventions_section = String::new();
        if !ctx.agents_md_summaries.is_empty() {
            conventions_section.push_str("## Project Conventions (use read_agents_md(path) for full text)\n");
            for (path, heading_summary) in &ctx.agents_md_summaries {
                conventions_section.push_str(&format!("- {}: {}\n", path, heading_summary));
            }
            conventions_section.push('\n');
        }

        // Add sections in priority order if they fit
        // Priority 1: Session Continuity
        let session_tokens = estimate_tokens(&session_section);
        if session_tokens > 0 && session_tokens <= remaining_budget {
            prompt.push_str(&session_section);
            remaining_budget -= session_tokens;
        }

        // Priority 2: Active Decisions
        let decisions_tokens = estimate_tokens(&decisions_section);
        if decisions_tokens > 0 && decisions_tokens <= remaining_budget {
            prompt.push_str(&decisions_section);
            remaining_budget -= decisions_tokens;
        }

        // Priority 3: Relevant Observations
        let observations_tokens = estimate_tokens(&observations_section);
        if observations_tokens > 0 && observations_tokens <= remaining_budget {
            prompt.push_str(&observations_section);
            remaining_budget -= observations_tokens;
        }

        // Priority 4: Project Conventions
        let conventions_tokens = estimate_tokens(&conventions_section);
        if conventions_tokens > 0 && conventions_tokens <= remaining_budget {
            prompt.push_str(&conventions_section);
        }

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
            previous_attempt: None,
            dependency_statuses: vec![],
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

    #[test]
    fn test_v2_phase5_ac3_1_dependency_status_done() {
        // v2-phase5.AC3.1: System prompt includes [DEP:DONE] lines for completed dependencies
        use crate::agent::profile::AgentProfile;
        use crate::graph::store::GraphStore;
        use crate::graph::{GraphNode, NodeType};
        use crate::security::SecurityScope;
        use anyhow::Result;
        use async_trait::async_trait;
        use std::collections::HashMap;
        use std::sync::Arc;

        struct TestGraphStore;

        #[async_trait]
        impl GraphStore for TestGraphStore {
            async fn create_node(&self, _node: &GraphNode) -> Result<()> {
                Ok(())
            }
            async fn update_node(
                &self,
                _id: &str,
                _status: Option<crate::graph::NodeStatus>,
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

        let profile = AgentProfile {
            name: "test".to_string(),
            extends: None,
            role: "You are a helpful code assistant".to_string(),
            system_prompt: "Follow these rules carefully".to_string(),
            allowed_tools: vec![],
            security: SecurityScope::default(),
            llm: Default::default(),
            turn_limit: None,
            token_budget: None,
        };

        let ctx = AgentContext {
            work_package_tasks: vec![],
            relevant_decisions: vec![],
            handoff_notes: None,
            agents_md_summaries: vec![],
            profile,
            project_path: PathBuf::from("/test/project"),
            graph_store: Arc::new(TestGraphStore),
            previous_attempt: None,
            dependency_statuses: vec![
                ("ra-1234".to_string(), "Setup database".to_string(), true),
                ("ra-5678".to_string(), "Configure auth".to_string(), false),
            ],
        };

        let prompt = ContextBuilder::build_system_prompt(&ctx);

        assert!(
            prompt.contains("[DEP:DONE] ra-1234 → Setup database (completed)"),
            "Should contain completed dependency with [DEP:DONE]"
        );
        assert!(
            prompt.contains("[DEP:PENDING] ra-5678 → Configure auth (pending)"),
            "Should contain pending dependency with [DEP:PENDING]"
        );
    }

    #[test]
    fn test_v2_phase5_ac3_2_previous_attempt_present() {
        // v2-phase5.AC3.2: System prompt includes [PREV_ATTEMPT] section when previous_attempt is Some
        use crate::agent::profile::AgentProfile;
        use crate::graph::store::GraphStore;
        use crate::graph::{GraphNode, NodeType};
        use crate::security::SecurityScope;
        use anyhow::Result;
        use async_trait::async_trait;
        use std::collections::HashMap;
        use std::sync::Arc;

        struct TestGraphStore;

        #[async_trait]
        impl GraphStore for TestGraphStore {
            async fn create_node(&self, _node: &GraphNode) -> Result<()> {
                Ok(())
            }
            async fn update_node(
                &self,
                _id: &str,
                _status: Option<crate::graph::NodeStatus>,
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

        let profile = AgentProfile {
            name: "test".to_string(),
            extends: None,
            role: "You are a helpful code assistant".to_string(),
            system_prompt: "Follow these rules carefully".to_string(),
            allowed_tools: vec![],
            security: SecurityScope::default(),
            llm: Default::default(),
            turn_limit: None,
            token_budget: None,
        };

        let ctx = AgentContext {
            work_package_tasks: vec![],
            relevant_decisions: vec![],
            handoff_notes: None,
            agents_md_summaries: vec![],
            profile,
            project_path: PathBuf::from("/test/project"),
            graph_store: Arc::new(TestGraphStore),
            previous_attempt: Some("Previous attempt failed: file not found".to_string()),
            dependency_statuses: vec![],
        };

        let prompt = ContextBuilder::build_system_prompt(&ctx);

        assert!(
            prompt.contains("## Previous Attempt"),
            "Should contain Previous Attempt section when previous_attempt is Some"
        );
        assert!(
            prompt.contains("[PREV_ATTEMPT] Previous attempt failed: file not found"),
            "Should contain previous attempt details with [PREV_ATTEMPT] marker"
        );
    }

    #[test]
    fn test_v2_phase5_ac3_3_previous_attempt_absent() {
        // v2-phase5.AC3.3: System prompt omits Previous Attempt section entirely when no previous attempt exists
        use crate::agent::profile::AgentProfile;
        use crate::graph::store::GraphStore;
        use crate::graph::{GraphNode, NodeType};
        use crate::security::SecurityScope;
        use anyhow::Result;
        use async_trait::async_trait;
        use std::collections::HashMap;
        use std::sync::Arc;

        struct TestGraphStore;

        #[async_trait]
        impl GraphStore for TestGraphStore {
            async fn create_node(&self, _node: &GraphNode) -> Result<()> {
                Ok(())
            }
            async fn update_node(
                &self,
                _id: &str,
                _status: Option<crate::graph::NodeStatus>,
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

        let profile = AgentProfile {
            name: "test".to_string(),
            extends: None,
            role: "You are a helpful code assistant".to_string(),
            system_prompt: "Follow these rules carefully".to_string(),
            allowed_tools: vec![],
            security: SecurityScope::default(),
            llm: Default::default(),
            turn_limit: None,
            token_budget: None,
        };

        let ctx = AgentContext {
            work_package_tasks: vec![],
            relevant_decisions: vec![],
            handoff_notes: None,
            agents_md_summaries: vec![],
            profile,
            project_path: PathBuf::from("/test/project"),
            graph_store: Arc::new(TestGraphStore),
            previous_attempt: None,
            dependency_statuses: vec![],
        };

        let prompt = ContextBuilder::build_system_prompt(&ctx);

        assert!(
            !prompt.contains("## Previous Attempt"),
            "Should NOT contain Previous Attempt section when previous_attempt is None"
        );
    }

    #[test]
    fn test_v2_phase5_ac4_1_budget_aware_trimming() {
        // v2-phase5.AC4.1: With a very small budget, only required sections appear; optional sections are trimmed
        use crate::agent::profile::AgentProfile;
        use crate::graph::store::GraphStore;
        use crate::graph::{GraphNode, NodeType};
        use crate::security::SecurityScope;
        use anyhow::Result;
        use async_trait::async_trait;
        use std::collections::HashMap;
        use std::sync::Arc;

        struct TestGraphStore;

        #[async_trait]
        impl GraphStore for TestGraphStore {
            async fn create_node(&self, _node: &GraphNode) -> Result<()> {
                Ok(())
            }
            async fn update_node(
                &self,
                _id: &str,
                _status: Option<crate::graph::NodeStatus>,
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

        let profile = AgentProfile {
            name: "test".to_string(),
            extends: None,
            role: "You are a helpful code assistant".to_string(),
            system_prompt: "Follow these rules carefully".to_string(),
            allowed_tools: vec![],
            security: SecurityScope::default(),
            llm: Default::default(),
            turn_limit: None,
            token_budget: None,
        };

        // Create tasks with long descriptions to trigger trimming
        let work_package_tasks = vec![GraphNode {
            id: "task-1".to_string(),
            project_id: "proj-1".to_string(),
            node_type: NodeType::Task,
            title: "Implement feature".to_string(),
            description: "This is a very long description that should be trimmed when budget is tight. It contains multiple sentences and spans several lines of text to ensure we have enough content to test budget trimming behavior.".to_string(),
            status: crate::graph::NodeStatus::Ready,
            priority: Some(crate::graph::Priority::High),
            assigned_to: None,
            created_by: None,
            labels: vec![],
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            blocked_reason: None,
            metadata: HashMap::new(),
        }];

        // Create decisions
        let relevant_decisions = vec![GraphNode {
            id: "decision-1".to_string(),
            project_id: "proj-1".to_string(),
            node_type: NodeType::Decision,
            title: "Architecture decision".to_string(),
            description: "Choose the right architecture for the system by considering scalability, maintainability, and performance requirements.".to_string(),
            status: crate::graph::NodeStatus::Decided,
            priority: None,
            assigned_to: None,
            created_by: None,
            labels: vec![],
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            blocked_reason: None,
            metadata: {
                let mut m = HashMap::new();
                m.insert("chosen_option".to_string(), "Option B".to_string());
                m
            },
        }];

        let ctx = AgentContext {
            work_package_tasks,
            relevant_decisions,
            handoff_notes: Some("Previous session notes that are quite detailed and span multiple concepts".to_string()),
            agents_md_summaries: vec![(
                "src/AGENTS.md".to_string(),
                "Code standards and conventions for the project".to_string(),
            )],
            profile,
            project_path: PathBuf::from("/test/project"),
            graph_store: Arc::new(TestGraphStore),
            previous_attempt: None,
            dependency_statuses: vec![],
        };

        // Test with very small budget (100 tokens - forces trimming of optional sections)
        let small_budget = ContextBudget {
            max_tokens: 100,
        };

        let prompt = ContextBuilder::build_system_prompt_with_budget(&ctx, &small_budget);

        // Required sections should always be present
        assert!(prompt.contains("## Role"), "Should contain Role section (required)");
        assert!(
            prompt.contains("## Task"),
            "Should contain Task section (required)"
        );
        assert!(prompt.contains("## Rules"), "Should contain Rules section (required)");

        // With tiny budget, optional sections should be trimmed
        // Project Conventions is lowest priority and should be trimmed
        assert!(
            !prompt.contains("## Project Conventions"),
            "Should trim Project Conventions (lowest priority) with very small budget"
        );
    }

    #[test]
    fn test_v2_phase5_ac4_2_required_sections_never_trimmed() {
        // v2-phase5.AC4.2: Required sections (Role, Task, Rules) are never trimmed regardless of budget
        use crate::agent::profile::AgentProfile;
        use crate::graph::store::GraphStore;
        use crate::graph::{GraphNode, NodeType};
        use crate::security::SecurityScope;
        use anyhow::Result;
        use async_trait::async_trait;
        use std::collections::HashMap;
        use std::sync::Arc;

        struct TestGraphStore;

        #[async_trait]
        impl GraphStore for TestGraphStore {
            async fn create_node(&self, _node: &GraphNode) -> Result<()> {
                Ok(())
            }
            async fn update_node(
                &self,
                _id: &str,
                _status: Option<crate::graph::NodeStatus>,
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

        let profile = AgentProfile {
            name: "test".to_string(),
            extends: None,
            role: "You are a helpful code assistant".to_string(),
            system_prompt: "Follow these rules carefully".to_string(),
            allowed_tools: vec![],
            security: SecurityScope::default(),
            llm: Default::default(),
            turn_limit: None,
            token_budget: None,
        };

        let work_package_tasks = vec![GraphNode {
            id: "task-1".to_string(),
            project_id: "proj-1".to_string(),
            node_type: NodeType::Task,
            title: "Implement feature".to_string(),
            description: "A detailed implementation task".to_string(),
            status: crate::graph::NodeStatus::Ready,
            priority: Some(crate::graph::Priority::High),
            assigned_to: None,
            created_by: None,
            labels: vec![],
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            blocked_reason: None,
            metadata: HashMap::new(),
        }];

        let ctx = AgentContext {
            work_package_tasks,
            relevant_decisions: vec![],
            handoff_notes: None,
            agents_md_summaries: vec![],
            profile,
            project_path: PathBuf::from("/test/project"),
            graph_store: Arc::new(TestGraphStore),
            previous_attempt: None,
            dependency_statuses: vec![],
        };

        // Test with extremely small budget (10 tokens - smaller than required sections)
        let tiny_budget = ContextBudget {
            max_tokens: 10,
        };

        let prompt = ContextBuilder::build_system_prompt_with_budget(&ctx, &tiny_budget);

        // Required sections must always be present, even with a tiny budget
        assert!(
            prompt.contains("## Role"),
            "Should contain Role section even with tiny budget (required)"
        );
        assert!(
            prompt.contains("## Task"),
            "Should contain Task section even with tiny budget (required)"
        );
        assert!(
            prompt.contains("## Rules"),
            "Should contain Rules section even with tiny budget (required)"
        );
    }

    #[test]
    fn test_context_budget_default() {
        let budget = ContextBudget::default();
        assert_eq!(budget.max_tokens, 4000, "Default budget should be 4000 tokens");
    }

    #[test]
    fn test_context_budget_custom() {
        let budget = ContextBudget { max_tokens: 2000 };
        assert_eq!(budget.max_tokens, 2000);
    }
}
