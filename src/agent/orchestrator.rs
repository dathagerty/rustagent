use crate::agent::profile::resolve_profile;
use crate::agent::runtime::{AgentRuntime, RuntimeConfig};
use crate::agent::work_package::{
    FileOwnershipMap, TaskForGrouping, WorkPackage, WorkerHandle, WorkerState,
    generate_work_package_id, group_tasks_into_packages,
};
use crate::agent::worktree::WorktreeManager;
use crate::agent::{AgentContext, AgentId, AgentOutcome};
use crate::context::resolve_agents_md;
use crate::graph::store::{GraphStore, NodeQuery};
use crate::graph::{
    GraphNode, NodeStatus, NodeType, Priority, generate_child_id, generate_goal_id,
};
use crate::llm::LlmClient;
use crate::message::{MessageBus, WorkerMessage};
use crate::security::SecurityValidator;
use crate::security::permission::PermissionHandler;
use crate::tools::factory::create_v2_registry;
use anyhow::{Result, anyhow};
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Configuration for the orchestrator.
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Maximum number of concurrent workers (default: 4)
    pub max_concurrent_workers: usize,
    /// Maximum retries per failed task (default: 2)
    pub max_retries_per_task: usize,
    /// Maximum turns per worker (default: 100)
    pub worker_turn_limit: usize,
    /// Worker progress report interval in turns (default: 10)
    pub check_in_interval: usize,
    /// Whether to spawn a reviewer after each coder completes
    pub review_required: bool,
    /// Max consecutive LLM failures before blocking a worker (default: 3)
    pub max_consecutive_llm_failures: usize,
    /// Max consecutive tool failures before blocking a worker (default: 3)
    pub max_consecutive_tool_failures: usize,
    /// Per-worker token budget (default: 200_000)
    pub worker_token_budget: usize,
    /// Token budget warning threshold as percentage (default: 80)
    pub token_budget_warning_pct: u8,
    /// Optional goal-level token budget (pause + approval if exceeded)
    pub max_tokens_per_goal: Option<usize>,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_workers: 4,
            max_retries_per_task: 2,
            worker_turn_limit: 100,
            check_in_interval: 10,
            review_required: false,
            max_consecutive_llm_failures: 3,
            max_consecutive_tool_failures: 3,
            worker_token_budget: 200_000,
            token_budget_warning_pct: 80,
            max_tokens_per_goal: None,
        }
    }
}

/// State of the orchestrator state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrchestratorState {
    /// Load config, connect DB, check for interrupted session
    Startup,
    /// Load or create goal from user input
    Loading,
    /// Spawn planner worker to create initial task breakdown
    Planning,
    /// Query ready tasks, group into work packages, spawn workers
    Scheduling,
    /// Wait for worker messages (progress, completion, blocks)
    Monitoring,
    /// Spawn reviewer workers if review_required
    Reviewing,
    /// Generate session summary, report results
    Completing,
}

/// Result returned by the orchestrator on completion.
#[derive(Debug)]
pub struct OrchestratorResult {
    pub goal_id: String,
    pub session_id: Option<String>,
    pub cumulative_tokens: usize,
    pub summary: String,
}

/// The orchestrator: a deterministic state machine that coordinates workers.
///
/// NOT an LLM agent — it follows rules and doesn't burn tokens on coordination.
pub struct Orchestrator {
    config: OrchestratorConfig,
    state: OrchestratorState,
    graph_store: Arc<dyn GraphStore>,
    message_bus: Arc<dyn MessageBus>,
    llm_client: Arc<dyn LlmClient>,
    security_validator: Arc<SecurityValidator>,
    permission_handler: Arc<dyn PermissionHandler>,
    project_path: PathBuf,
    project_id: String,
    active_workers: HashMap<AgentId, WorkerHandle>,
    file_locks: FileOwnershipMap,
    worktree_manager: Option<WorktreeManager>,
    goal_id: Option<String>,
    session_id: Option<String>,
    cumulative_tokens: usize,
}

impl Orchestrator {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: OrchestratorConfig,
        graph_store: Arc<dyn GraphStore>,
        message_bus: Arc<dyn MessageBus>,
        llm_client: Arc<dyn LlmClient>,
        security_validator: Arc<SecurityValidator>,
        permission_handler: Arc<dyn PermissionHandler>,
        project_path: PathBuf,
        project_id: String,
    ) -> Self {
        let worktree_manager = if config.max_concurrent_workers > 1 {
            Some(WorktreeManager::new(project_path.clone()))
        } else {
            None
        };

        Self {
            config,
            state: OrchestratorState::Startup,
            graph_store,
            message_bus,
            llm_client,
            security_validator,
            permission_handler,
            project_path,
            project_id,
            active_workers: HashMap::new(),
            file_locks: FileOwnershipMap::new(),
            worktree_manager,
            goal_id: None,
            session_id: None,
            cumulative_tokens: 0,
        }
    }

    /// Get the current state.
    pub fn state(&self) -> &OrchestratorState {
        &self.state
    }

    /// Get the config.
    pub fn config(&self) -> &OrchestratorConfig {
        &self.config
    }

    /// Get the goal ID.
    pub fn goal_id(&self) -> Option<&str> {
        self.goal_id.as_deref()
    }

    /// Get the session ID.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Get the cumulative token count.
    pub fn cumulative_tokens(&self) -> usize {
        self.cumulative_tokens
    }

    /// Get the number of active workers.
    pub fn active_worker_count(&self) -> usize {
        self.active_workers.len()
    }

    /// Run the orchestrator to completion for a given goal.
    pub async fn run(&mut self, goal_description: &str) -> Result<OrchestratorResult> {
        self.run_with_shutdown(goal_description, CancellationToken::new())
            .await
    }

    /// Run the orchestrator with a shutdown token for graceful cancellation.
    ///
    /// If the token is cancelled (e.g. via Ctrl+C), active workers are stopped
    /// and the current session state is preserved for later recovery.
    pub async fn run_with_shutdown(
        &mut self,
        goal_description: &str,
        shutdown_token: CancellationToken,
    ) -> Result<OrchestratorResult> {
        loop {
            if shutdown_token.is_cancelled() {
                return self.handle_graceful_shutdown().await;
            }

            match &self.state {
                OrchestratorState::Startup => {
                    self.state = self.handle_startup().await?;
                }
                OrchestratorState::Loading => {
                    self.state = self.handle_loading(goal_description).await?;
                }
                OrchestratorState::Planning => {
                    self.state = self.handle_planning().await?;
                }
                OrchestratorState::Scheduling => {
                    self.state = self.handle_scheduling().await?;
                }
                OrchestratorState::Monitoring => {
                    self.state = self.handle_monitoring().await?;
                }
                OrchestratorState::Reviewing => {
                    self.state = self.handle_reviewing().await?;
                }
                OrchestratorState::Completing => {
                    return self.handle_completing().await;
                }
            }
        }
    }

    /// Set the goal ID (for testing and manual orchestration).
    pub fn set_goal_id(&mut self, goal_id: Option<String>) {
        self.goal_id = goal_id;
    }

    /// Set cumulative tokens (for testing and recovery).
    pub fn set_cumulative_tokens(&mut self, tokens: usize) {
        self.cumulative_tokens = tokens;
    }

    // ===== State Handlers =====

    /// Check for interrupted sessions; if found, reset InProgress tasks to Ready.
    pub async fn handle_startup(&mut self) -> Result<OrchestratorState> {
        // Check for interrupted sessions by looking for InProgress tasks under active goals
        let in_progress_tasks = self
            .graph_store
            .query_nodes(&NodeQuery {
                node_type: Some(NodeType::Task),
                status: Some(NodeStatus::InProgress),
                project_id: Some(self.project_id.clone()),
                parent_id: None,
                query: None,
            })
            .await?;

        if !in_progress_tasks.is_empty() {
            // Reset InProgress tasks to Ready for re-scheduling
            for task in &in_progress_tasks {
                self.graph_store
                    .update_node(&task.id, Some(NodeStatus::Ready), None, None, None, None)
                    .await?;
            }
        }

        // Also check for Claimed tasks that were never started
        let claimed_tasks = self
            .graph_store
            .query_nodes(&NodeQuery {
                node_type: Some(NodeType::Task),
                status: Some(NodeStatus::Claimed),
                project_id: Some(self.project_id.clone()),
                parent_id: None,
                query: None,
            })
            .await?;

        for task in &claimed_tasks {
            self.graph_store
                .update_node(&task.id, Some(NodeStatus::Ready), None, None, None, None)
                .await?;
        }

        Ok(OrchestratorState::Loading)
    }

    /// Load or create goal node and session.
    pub async fn handle_loading(&mut self, goal_description: &str) -> Result<OrchestratorState> {
        // Check for an existing active goal for this project
        let active_goals = self
            .graph_store
            .query_nodes(&NodeQuery {
                node_type: Some(NodeType::Goal),
                status: Some(NodeStatus::Active),
                project_id: Some(self.project_id.clone()),
                parent_id: None,
                query: None,
            })
            .await?;

        if let Some(goal) = active_goals.first() {
            // Resume existing goal
            self.goal_id = Some(goal.id.clone());

            // Create/reuse goal branch if multi-agent mode
            if let Some(ref wm) = self.worktree_manager
                && let Err(e) = wm.create_goal_branch(&goal.id)
            {
                tracing::warn!(
                    "Failed to create goal branch (continuing without worktrees): {}",
                    e
                );
            }

            // Check if tasks already exist under this goal
            let children = self.graph_store.get_children(&goal.id).await?;
            let has_tasks = children
                .iter()
                .any(|(node, _)| node.node_type == NodeType::Task);

            return if has_tasks {
                Ok(OrchestratorState::Scheduling)
            } else {
                Ok(OrchestratorState::Planning)
            };
        }

        // Create a new goal
        let goal_id = generate_goal_id();
        let goal_node = GraphNode {
            id: goal_id.clone(),
            project_id: self.project_id.clone(),
            node_type: NodeType::Goal,
            title: goal_description.to_string(),
            description: goal_description.to_string(),
            status: NodeStatus::Active,
            priority: Some(Priority::High),
            assigned_to: None,
            created_by: Some("orchestrator".to_string()),
            labels: vec![],
            created_at: Utc::now(),
            started_at: Some(Utc::now()),
            completed_at: None,
            blocked_reason: None,
            metadata: HashMap::new(),
        };
        self.graph_store.create_node(&goal_node).await?;
        self.goal_id = Some(goal_id.clone());

        // Create goal branch if multi-agent mode
        if let Some(ref wm) = self.worktree_manager
            && let Err(e) = wm.create_goal_branch(&goal_id)
        {
            tracing::warn!(
                "Failed to create goal branch (continuing without worktrees): {}",
                e
            );
        }

        Ok(OrchestratorState::Planning)
    }

    /// Spawn a planner worker to create task breakdown, then wait for it.
    pub async fn handle_planning(&mut self) -> Result<OrchestratorState> {
        let goal_id = self
            .goal_id
            .as_ref()
            .ok_or_else(|| anyhow!("no goal_id set in Planning state"))?
            .clone();

        // Get the goal node for the planner's work package
        let goal_node = self
            .graph_store
            .get_node(&goal_id)
            .await?
            .ok_or_else(|| anyhow!("goal node not found: {}", goal_id))?;

        // Create a work package with the goal as the sole task
        let planner_package = WorkPackage {
            id: generate_work_package_id(),
            task_ids: vec![goal_id.clone()],
            file_scope: vec![],
            profile: "planner".to_string(),
            priority: Priority::High,
            estimated_complexity: crate::agent::work_package::Complexity::Medium,
        };

        // Spawn the planner worker
        let worker_id = self.spawn_worker(planner_package, vec![goal_node]).await?;

        // Wait for the planner to complete (blocking — only one worker in Planning)
        let outcome = self.wait_for_worker(&worker_id).await?;

        // Accumulate tokens
        match &outcome {
            AgentOutcome::Completed { tokens_used, .. }
            | AgentOutcome::TokenBudgetExhausted { tokens_used, .. } => {
                self.cumulative_tokens += tokens_used;
            }
            _ => {}
        }

        // Clean up the worker
        self.file_locks.release(&worker_id);
        self.active_workers.remove(&worker_id);
        self.message_bus.remove_subscriber(&worker_id);

        // Check if planner created task nodes under the goal
        let children = self.graph_store.get_children(&goal_id).await?;
        let has_tasks = children
            .iter()
            .any(|(node, _)| node.node_type == NodeType::Task);

        if has_tasks {
            Ok(OrchestratorState::Scheduling)
        } else {
            Err(anyhow!(
                "Planner did not create any tasks under goal {}",
                goal_id
            ))
        }
    }

    /// Query ready tasks, group into work packages, spawn workers.
    pub async fn handle_scheduling(&mut self) -> Result<OrchestratorState> {
        let goal_id = self
            .goal_id
            .as_ref()
            .ok_or_else(|| anyhow!("no goal_id set in Scheduling state"))?
            .clone();

        // Check goal-level token budget
        if let Some(max) = self.config.max_tokens_per_goal
            && self.cumulative_tokens >= max
        {
            return Ok(OrchestratorState::Completing);
        }

        // Query ready tasks under the goal
        let ready_tasks = self.graph_store.get_ready_tasks(&goal_id).await?;

        // No ready tasks and no active workers — done
        if ready_tasks.is_empty() && self.active_workers.is_empty() {
            return if self.config.review_required {
                Ok(OrchestratorState::Reviewing)
            } else {
                Ok(OrchestratorState::Completing)
            };
        }

        // No ready tasks but workers still active — monitor existing
        if ready_tasks.is_empty() {
            return Ok(OrchestratorState::Monitoring);
        }

        // Convert graph nodes to TaskForGrouping structs
        let tasks_for_grouping: Vec<TaskForGrouping> = ready_tasks
            .iter()
            .map(|node| {
                let file_scope = node
                    .metadata
                    .get("file_scope")
                    .map(|s| {
                        s.split(',')
                            .filter(|p| !p.is_empty())
                            .map(|p| PathBuf::from(p.trim()))
                            .collect()
                    })
                    .unwrap_or_default();

                let profile = node
                    .metadata
                    .get("profile")
                    .cloned()
                    .unwrap_or_else(|| "coder".to_string());

                let depends_on = node
                    .metadata
                    .get("depends_on")
                    .map(|s| {
                        s.split(',')
                            .filter(|p| !p.is_empty())
                            .map(|p| p.trim().to_string())
                            .collect()
                    })
                    .unwrap_or_default();

                TaskForGrouping {
                    task_id: node.id.clone(),
                    file_scope,
                    profile,
                    priority: node.priority.unwrap_or(Priority::Medium),
                    depends_on,
                }
            })
            .collect();

        // Group into work packages
        let packages = group_tasks_into_packages(tasks_for_grouping);

        // Spawn workers up to available capacity
        let available_slots = self
            .config
            .max_concurrent_workers
            .saturating_sub(self.active_workers.len());

        for package in packages.into_iter().take(available_slots) {
            // Try to acquire file locks for this package
            let worker_id = format!("worker-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);

            if self
                .file_locks
                .acquire(&worker_id, &package.file_scope)
                .is_ok()
            {
                // Claim all tasks in the package
                let mut claimed_all = true;
                for task_id in &package.task_ids {
                    if !self.graph_store.claim_task(task_id, &worker_id).await? {
                        claimed_all = false;
                        break;
                    }
                }

                if claimed_all {
                    // Get the task nodes for context
                    let mut task_nodes = Vec::new();
                    for task_id in &package.task_ids {
                        if let Some(node) = self.graph_store.get_node(task_id).await? {
                            task_nodes.push(node);
                        }
                    }

                    self.spawn_worker_with_id(worker_id, package, task_nodes)
                        .await?;
                } else {
                    // Release locks if we couldn't claim all tasks
                    self.file_locks.release(&worker_id);
                }
            }
            // If file conflict, skip — will be picked up in next scheduling pass
        }

        Ok(OrchestratorState::Monitoring)
    }

    /// Monitor active workers for messages and completions.
    pub async fn handle_monitoring(&mut self) -> Result<OrchestratorState> {
        let mut rx = self.message_bus.subscribe(&"orchestrator".to_string());

        loop {
            // Wait for a message or a short timeout
            tokio::select! {
                Some(msg) = rx.recv() => {
                    self.process_worker_message(msg).await?;
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
            }

            // Poll for completed JoinHandles
            self.poll_completed_workers().await?;

            // Check if we should reschedule
            if self.active_workers.is_empty() {
                // All workers done — go back to scheduling to check for more work
                self.message_bus
                    .remove_subscriber(&"orchestrator".to_string());
                return Ok(OrchestratorState::Scheduling);
            }
        }
    }

    /// Spawn reviewer workers for completed work if review_required.
    pub async fn handle_reviewing(&mut self) -> Result<OrchestratorState> {
        if !self.config.review_required {
            return Ok(OrchestratorState::Completing);
        }

        let goal_id = self
            .goal_id
            .as_ref()
            .ok_or_else(|| anyhow!("no goal_id in Reviewing state"))?
            .clone();

        // Find completed tasks that haven't been reviewed yet
        let all_tasks = self.graph_store.get_subtree(&goal_id).await?;
        let unreviewed: Vec<&GraphNode> = all_tasks
            .iter()
            .filter(|n| {
                n.node_type == NodeType::Task
                    && n.status == NodeStatus::Completed
                    && !n.metadata.contains_key("reviewed")
            })
            .collect();

        if unreviewed.is_empty() {
            return Ok(OrchestratorState::Completing);
        }

        // Create a single review work package for all unreviewed tasks
        let task_ids: Vec<String> = unreviewed.iter().map(|t| t.id.clone()).collect();
        let task_nodes: Vec<GraphNode> = unreviewed.into_iter().cloned().collect();
        let file_scope: Vec<PathBuf> = task_nodes
            .iter()
            .flat_map(|t| {
                t.metadata
                    .get("file_scope")
                    .map(|s| {
                        s.split(',')
                            .filter(|p| !p.is_empty())
                            .map(|p| PathBuf::from(p.trim()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .collect();

        let review_package = WorkPackage {
            id: generate_work_package_id(),
            task_ids,
            file_scope,
            profile: "reviewer".to_string(),
            priority: Priority::High,
            estimated_complexity: crate::agent::work_package::Complexity::Medium,
        };

        self.spawn_worker(review_package, task_nodes).await?;

        // Go to Monitoring to wait for reviewer
        Ok(OrchestratorState::Monitoring)
    }

    /// Generate session summary and return result.
    pub async fn handle_completing(&mut self) -> Result<OrchestratorResult> {
        let goal_id = self
            .goal_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        // Mark goal as completed
        let _ = self
            .graph_store
            .update_node(
                &goal_id,
                Some(NodeStatus::Completed),
                None,
                None,
                None,
                None,
            )
            .await;

        // Build summary from completed tasks
        let mut summary = format!("Goal {} completed.\n", goal_id);
        if let Ok(subtree) = self.graph_store.get_subtree(&goal_id).await {
            let completed = subtree
                .iter()
                .filter(|n| n.node_type == NodeType::Task && n.status == NodeStatus::Completed)
                .count();
            let failed = subtree
                .iter()
                .filter(|n| n.node_type == NodeType::Task && n.status == NodeStatus::Failed)
                .count();
            let blocked = subtree
                .iter()
                .filter(|n| n.node_type == NodeType::Task && n.status == NodeStatus::Blocked)
                .count();

            summary.push_str(&format!(
                "Tasks: {} completed, {} failed, {} blocked\n",
                completed, failed, blocked
            ));
            summary.push_str(&format!("Total tokens used: {}\n", self.cumulative_tokens));
        }

        // If multi-agent mode, report the goal branch
        if self.worktree_manager.is_some() {
            let branch = WorktreeManager::goal_branch_name(&goal_id);
            summary.push_str(&format!("Changes are on branch: {}\n", branch));
        }

        Ok(OrchestratorResult {
            goal_id,
            session_id: self.session_id.clone(),
            cumulative_tokens: self.cumulative_tokens,
            summary,
        })
    }

    /// Handle graceful shutdown: cancel workers, save state, return result.
    async fn handle_graceful_shutdown(&mut self) -> Result<OrchestratorResult> {
        // Cancel all active workers
        for (worker_id, handle) in &self.active_workers {
            tracing::info!("Cancelling worker {}", worker_id);
            handle.cancel_token.cancel();
        }

        // Wait briefly for workers to finish (best-effort)
        if !self.active_workers.is_empty() {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        // Reset any InProgress tasks back to Ready for recovery
        if let Some(goal_id) = &self.goal_id
            && let Ok(subtree) = self.graph_store.get_subtree(goal_id).await
        {
            for node in &subtree {
                if node.node_type == NodeType::Task
                    && (node.status == NodeStatus::InProgress || node.status == NodeStatus::Claimed)
                {
                    let _ = self
                        .graph_store
                        .update_node(&node.id, Some(NodeStatus::Ready), None, None, None, None)
                        .await;
                }
            }
        }

        let goal_id = self
            .goal_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        let summary = format!(
            "Shutdown: goal {} interrupted. InProgress tasks reset to Ready. \
             Tokens used: {}. Re-run to resume.",
            goal_id, self.cumulative_tokens
        );

        Ok(OrchestratorResult {
            goal_id,
            session_id: self.session_id.clone(),
            cumulative_tokens: self.cumulative_tokens,
            summary,
        })
    }

    // ===== Worker Management =====

    /// Spawn a worker for a work package. Returns the worker's AgentId.
    async fn spawn_worker(
        &mut self,
        package: WorkPackage,
        task_nodes: Vec<GraphNode>,
    ) -> Result<AgentId> {
        let worker_id = format!("worker-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
        self.spawn_worker_with_id(worker_id.clone(), package, task_nodes)
            .await?;
        Ok(worker_id)
    }

    /// Spawn a worker with a specific ID.
    async fn spawn_worker_with_id(
        &mut self,
        worker_id: AgentId,
        package: WorkPackage,
        task_nodes: Vec<GraphNode>,
    ) -> Result<()> {
        let profile_name = package.profile.clone();

        // Resolve the agent profile
        let profile = resolve_profile(&profile_name, Some(&self.project_path))?;

        // Get active decisions for context
        let decisions = self
            .graph_store
            .get_active_decisions(&self.project_id)
            .await
            .unwrap_or_default();

        // Resolve AGENTS.md summaries (empty file_scope for planner, actual scope for workers)
        let file_scope: Vec<PathBuf> = package.file_scope.clone();
        let agents_md_summaries =
            resolve_agents_md(&self.project_path, &file_scope).unwrap_or_default();

        // Determine worker's project path (worktree in multi-agent, original in single-agent)
        let worker_project_path =
            if let (Some(wm), Some(goal_id)) = (&self.worktree_manager, &self.goal_id) {
                match wm.create_worktree(goal_id, &package.id) {
                    Ok(path) => path,
                    Err(e) => {
                        tracing::warn!("Failed to create worktree, falling back to main: {}", e);
                        self.project_path.clone()
                    }
                }
            } else {
                self.project_path.clone()
            };

        // Build AgentContext
        let ctx = AgentContext {
            work_package_tasks: task_nodes,
            relevant_decisions: decisions,
            handoff_notes: None,
            agents_md_summaries,
            profile: profile.clone(),
            project_path: worker_project_path,
            graph_store: self.graph_store.clone(),
            previous_attempt: None,
            dependency_statuses: vec![],
        };

        // Build RuntimeConfig from OrchestratorConfig
        let runtime_config = RuntimeConfig {
            max_turns: self.config.worker_turn_limit,
            max_consecutive_llm_failures: self.config.max_consecutive_llm_failures,
            max_consecutive_tool_failures: self.config.max_consecutive_tool_failures,
            token_budget: self.config.worker_token_budget,
            token_budget_warning_pct: self.config.token_budget_warning_pct,
            message_bus: Some(self.message_bus.clone()),
            agent_id: Some(worker_id.clone()),
            check_in_interval: self.config.check_in_interval,
        };

        // Create tool registry (multi-agent mode: pass message bus + worker ID)
        let registry = create_v2_registry(
            self.security_validator.clone(),
            self.permission_handler.clone(),
            self.graph_store.clone(),
            Some(self.message_bus.clone()),
            Some(worker_id.clone()),
        );

        // Create AgentRuntime
        let runtime = AgentRuntime::new(
            self.llm_client.clone(),
            registry,
            profile.clone(),
            runtime_config,
        );

        // Subscribe worker to message bus
        let _worker_rx = self.message_bus.subscribe(&worker_id);

        // Create cancellation token
        let cancel_token = CancellationToken::new();
        let cancel_clone = cancel_token.clone();

        // Spawn the worker task
        let handle = tokio::spawn(async move {
            tokio::select! {
                result = runtime.run(ctx) => result,
                _ = cancel_clone.cancelled() => {
                    Ok(AgentOutcome::Blocked { reason: "Cancelled by orchestrator".to_string() })
                }
            }
        });

        let now = Utc::now();
        let worker_handle = WorkerHandle {
            id: worker_id.clone(),
            profile: profile_name,
            work_package: package,
            state: WorkerState::Working,
            join_handle: handle,
            cancel_token,
            spawned_at: now,
            last_check_in: now,
        };

        self.active_workers.insert(worker_id, worker_handle);
        Ok(())
    }

    /// Wait for a specific worker to complete. Used in Planning state.
    async fn wait_for_worker(&mut self, worker_id: &AgentId) -> Result<AgentOutcome> {
        let handle = self
            .active_workers
            .get_mut(worker_id)
            .ok_or_else(|| anyhow!("worker {} not found", worker_id))?;

        // We need to take the JoinHandle out to await it
        // Use a placeholder that completes immediately
        let join_handle = std::mem::replace(
            &mut handle.join_handle,
            tokio::spawn(async {
                Ok(AgentOutcome::Completed {
                    summary: "placeholder".to_string(),
                    tokens_used: 0,
                })
            }),
        );

        match join_handle.await {
            Ok(Ok(outcome)) => {
                if let Some(wh) = self.active_workers.get_mut(worker_id) {
                    wh.state = WorkerState::Completed(outcome.clone());
                }
                Ok(outcome)
            }
            Ok(Err(e)) => {
                if let Some(wh) = self.active_workers.get_mut(worker_id) {
                    wh.state = WorkerState::Failed(e.to_string());
                }
                Err(e)
            }
            Err(join_err) => {
                if let Some(wh) = self.active_workers.get_mut(worker_id) {
                    wh.state = WorkerState::Failed(join_err.to_string());
                }
                Err(anyhow!("Worker task panicked: {}", join_err))
            }
        }
    }

    /// Poll all active workers for completed JoinHandles.
    async fn poll_completed_workers(&mut self) -> Result<()> {
        // Find workers whose JoinHandles are finished
        let finished_ids: Vec<AgentId> = self
            .active_workers
            .iter()
            .filter(|(_, wh)| wh.join_handle.is_finished())
            .map(|(id, _)| id.clone())
            .collect();

        for worker_id in finished_ids {
            if let Some(mut worker_handle) = self.active_workers.remove(&worker_id) {
                let task_ids = worker_handle.work_package.task_ids.clone();
                let wp_id = worker_handle.work_package.id.clone();
                let succeeded;

                match worker_handle.join_handle.await {
                    Ok(Ok(outcome)) => {
                        self.handle_worker_outcome(&worker_id, &task_ids, &outcome)
                            .await?;
                        worker_handle.state = WorkerState::Completed(outcome);
                        succeeded = true;
                    }
                    Ok(Err(e)) => {
                        let error_msg = e.to_string();
                        for task_id in &task_ids {
                            self.handle_task_retry_or_fail(task_id, &error_msg).await?;
                        }
                        worker_handle.state = WorkerState::Failed(error_msg);
                        succeeded = false;
                    }
                    Err(join_err) => {
                        let error_msg = format!("Worker panicked: {}", join_err);
                        for task_id in &task_ids {
                            self.handle_task_retry_or_fail(task_id, &error_msg).await?;
                        }
                        worker_handle.state = WorkerState::Failed(join_err.to_string());
                        succeeded = false;
                    }
                }

                // Merge and cleanup worktree on success; preserve on failure
                if let (Some(wm), Some(goal_id)) = (&self.worktree_manager, &self.goal_id) {
                    if succeeded {
                        if let Err(e) = wm.merge_work_package(goal_id, &wp_id) {
                            tracing::error!(
                                worker = %worker_id,
                                "Worktree merge failed (preserving for manual resolution): {}",
                                e
                            );
                        } else {
                            let _ = wm.cleanup_worktree(goal_id, &wp_id);
                        }
                    } else {
                        tracing::warn!(
                            worker = %worker_id,
                            "Worker failed — preserving worktree for debugging"
                        );
                    }
                }

                // Release file locks and message bus subscription
                self.file_locks.release(&worker_id);
                self.message_bus.remove_subscriber(&worker_id);
            }
        }

        Ok(())
    }

    /// Handle a successful worker outcome.
    pub async fn handle_worker_outcome(
        &mut self,
        worker_id: &AgentId,
        task_ids: &[String],
        outcome: &AgentOutcome,
    ) -> Result<()> {
        match outcome {
            AgentOutcome::Completed {
                tokens_used,
                summary,
            } => {
                self.cumulative_tokens += tokens_used;
                for task_id in task_ids {
                    let _ = self
                        .graph_store
                        .update_node(task_id, Some(NodeStatus::Completed), None, None, None, None)
                        .await;
                }
                tracing::info!(
                    worker = %worker_id,
                    tasks = ?task_ids,
                    tokens = tokens_used,
                    "Worker completed: {}",
                    summary
                );
            }
            AgentOutcome::Blocked { reason } => {
                for task_id in task_ids {
                    self.handle_task_retry_or_fail(task_id, reason).await?;
                }
            }
            AgentOutcome::Failed { error } => {
                for task_id in task_ids {
                    self.handle_task_retry_or_fail(task_id, error).await?;
                }
            }
            AgentOutcome::TokenBudgetExhausted {
                tokens_used,
                summary,
            } => {
                self.cumulative_tokens += tokens_used;
                // Treat as partial completion — mark tasks as ready for re-scheduling
                for task_id in task_ids {
                    let _ = self
                        .graph_store
                        .update_node(task_id, Some(NodeStatus::Ready), None, None, None, None)
                        .await;
                }
                tracing::warn!(
                    worker = %worker_id,
                    tokens = tokens_used,
                    "Worker token budget exhausted: {}",
                    summary
                );
            }
        }
        Ok(())
    }

    /// Handle retry logic for a failed task.
    pub async fn handle_task_retry_or_fail(&mut self, task_id: &str, error: &str) -> Result<()> {
        let node = self.graph_store.get_node(task_id).await?;
        let retry_count: usize = node
            .as_ref()
            .and_then(|n| n.metadata.get("retry_count"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        if retry_count < self.config.max_retries_per_task {
            // Retry: increment count and reset to Ready
            let mut metadata = node.map(|n| n.metadata.clone()).unwrap_or_default();
            metadata.insert("retry_count".to_string(), (retry_count + 1).to_string());

            self.graph_store
                .update_node(
                    task_id,
                    Some(NodeStatus::Ready),
                    None,
                    None,
                    None,
                    Some(&metadata),
                )
                .await?;

            tracing::info!(
                task = %task_id,
                retry = retry_count + 1,
                max = self.config.max_retries_per_task,
                "Task retry scheduled: {}",
                error
            );
        } else {
            // Exhausted retries — mark Failed
            self.graph_store
                .update_node(
                    task_id,
                    Some(NodeStatus::Failed),
                    None,
                    None,
                    Some(error),
                    None,
                )
                .await?;

            // Create an Observation node documenting the failure
            if let Some(goal_id) = &self.goal_id {
                let seq = self.graph_store.next_child_seq(goal_id).await?;
                let obs_id = generate_child_id(goal_id, seq);
                let obs = GraphNode {
                    id: obs_id,
                    project_id: self.project_id.clone(),
                    node_type: NodeType::Observation,
                    title: format!("Task {} failed after {} retries", task_id, retry_count),
                    description: error.to_string(),
                    status: NodeStatus::Active,
                    priority: None,
                    assigned_to: None,
                    created_by: Some("orchestrator".to_string()),
                    labels: vec!["failure".to_string()],
                    created_at: Utc::now(),
                    started_at: None,
                    completed_at: None,
                    blocked_reason: None,
                    metadata: HashMap::new(),
                };
                let _ = self.graph_store.create_node(&obs).await;
            }

            tracing::warn!(
                task = %task_id,
                retries = retry_count,
                "Task permanently failed: {}",
                error
            );
        }

        Ok(())
    }

    /// Process a message received from a worker.
    async fn process_worker_message(&mut self, msg: WorkerMessage) -> Result<()> {
        match msg {
            WorkerMessage::ProgressReport {
                agent_id,
                turn,
                summary: _,
            } => {
                if let Some(handle) = self.active_workers.get_mut(&agent_id) {
                    handle.last_check_in = Utc::now();
                    tracing::debug!(worker = %agent_id, turn = turn, "Worker check-in");
                }
            }
            WorkerMessage::TaskCompleted {
                agent_id,
                task_id,
                summary,
            } => {
                let _ = self
                    .graph_store
                    .update_node(
                        &task_id,
                        Some(NodeStatus::Completed),
                        None,
                        None,
                        None,
                        None,
                    )
                    .await;
                tracing::info!(
                    worker = %agent_id,
                    task = %task_id,
                    "Task completed: {}",
                    summary
                );
            }
            WorkerMessage::TaskBlocked {
                agent_id,
                task_id,
                reason,
            } => {
                let _ = self
                    .graph_store
                    .update_node(
                        &task_id,
                        Some(NodeStatus::Blocked),
                        None,
                        None,
                        Some(&reason),
                        None,
                    )
                    .await;
                tracing::warn!(
                    worker = %agent_id,
                    task = %task_id,
                    "Task blocked: {}",
                    reason
                );
            }
            WorkerMessage::NeedsDecision {
                agent_id,
                task_id: _,
                decision,
            } => {
                // Handle file scope expansion requests
                if let Some(requested_files) = decision.metadata.get("requested_files") {
                    let files: Vec<PathBuf> = requested_files
                        .split(',')
                        .filter(|p| !p.is_empty())
                        .map(|p| PathBuf::from(p.trim()))
                        .collect();

                    let can_expand = files
                        .iter()
                        .all(|f| self.file_locks.can_write(&agent_id, f));
                    if can_expand {
                        let _ = self.file_locks.acquire(&agent_id, &files);
                        let _ = self
                            .message_bus
                            .send(
                                &agent_id,
                                WorkerMessage::AdditionalContext {
                                    content: format!(
                                        "Scope expanded: you now have access to {}",
                                        requested_files
                                    ),
                                },
                            )
                            .await;
                    }
                }
            }
            WorkerMessage::NodeCreated { .. } => {
                // New nodes created by workers are discovered in next scheduling pass
            }
            _ => {
                // ReviewRequest, ReviewFeedback, Cancel, AdditionalContext
                // handled elsewhere or not relevant for orchestrator
            }
        }
        Ok(())
    }
}
