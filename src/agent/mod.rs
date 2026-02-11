pub mod builtin_profiles;
pub mod orchestrator;
pub mod profile;
pub mod runtime;
pub mod work_package;
pub mod worktree;

use crate::graph::GraphNode;
use crate::graph::store::GraphStore;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

pub use profile::AgentProfile;

/// Type alias for agent identifiers
pub type AgentId = String;

/// Agent trait defining the interface for executing work
#[async_trait]
pub trait Agent: Send + Sync {
    /// Get the agent's unique identifier
    fn id(&self) -> &AgentId;

    /// Get the agent's profile (configuration)
    fn profile(&self) -> &AgentProfile;

    /// Run the agent with the given context
    async fn run(&self, ctx: AgentContext) -> Result<AgentOutcome>;

    /// Cancel the agent's execution (no-op stub in Phase 1d)
    fn cancel(&self);
}

/// Outcome of an agent run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentOutcome {
    /// Task completed successfully
    Completed { summary: String, tokens_used: usize },

    /// Agent blocked due to unresolvable issues
    Blocked { reason: String },

    /// Task failed with error
    Failed { error: String },

    /// Token budget was exhausted
    TokenBudgetExhausted { summary: String, tokens_used: usize },
}

/// Context provided to an agent when running
#[derive(Clone)]
pub struct AgentContext {
    /// Tasks to work on in this package
    pub work_package_tasks: Vec<GraphNode>,

    /// Relevant decisions from the graph
    pub relevant_decisions: Vec<GraphNode>,

    /// Handoff notes from previous agent or orchestrator
    pub handoff_notes: Option<String>,

    /// Summaries extracted from AGENTS.md files (path, heading summary)
    pub agents_md_summaries: Vec<(String, String)>,

    /// Agent profile controlling behavior
    pub profile: AgentProfile,

    /// Project path for file operations
    pub project_path: PathBuf,

    /// Graph store for querying and updating nodes
    pub graph_store: Arc<dyn GraphStore>,
}

impl std::fmt::Debug for AgentContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentContext")
            .field("work_package_tasks", &self.work_package_tasks.len())
            .field("relevant_decisions", &self.relevant_decisions.len())
            .field("handoff_notes", &self.handoff_notes)
            .field("agents_md_summaries", &self.agents_md_summaries.len())
            .field("profile", &self.profile)
            .field("project_path", &self.project_path)
            .finish()
    }
}
