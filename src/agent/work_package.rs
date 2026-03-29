use crate::agent::{AgentId, AgentOutcome};
use crate::graph::Priority;
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Estimated complexity of a work package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Complexity {
    Small,  // 1-2 file changes, straightforward
    Medium, // Multiple files, some decision-making
    Large,  // Architectural changes, many files
}

/// A work package groups related tasks for a single worker.
#[derive(Debug, Clone)]
pub struct WorkPackage {
    pub id: String,
    pub task_ids: Vec<String>,
    pub file_scope: Vec<PathBuf>,
    pub profile: String,
    pub priority: Priority,
    pub estimated_complexity: Complexity,
}

/// Tracks file ownership to prevent concurrent modification conflicts.
#[derive(Debug, Default)]
pub struct FileOwnershipMap {
    locks: HashMap<PathBuf, AgentId>,
}

impl FileOwnershipMap {
    pub fn new() -> Self {
        Self {
            locks: HashMap::new(),
        }
    }

    /// Try to acquire ownership of files for an agent.
    /// Returns Err if any file is already owned by another agent.
    pub fn acquire(&mut self, agent_id: &AgentId, files: &[PathBuf]) -> Result<()> {
        // Pre-check: all files must be unowned or owned by this agent
        for file in files {
            if let Some(owner) = self.locks.get(file)
                && owner != agent_id
            {
                return Err(anyhow!(
                    "file {} is already owned by agent {}",
                    file.display(),
                    owner
                ));
            }
        }
        // All clear — acquire
        for file in files {
            self.locks.insert(file.clone(), agent_id.clone());
        }
        Ok(())
    }

    /// Release all files owned by an agent.
    pub fn release(&mut self, agent_id: &AgentId) {
        self.locks.retain(|_, owner| owner != agent_id);
    }

    /// Check if a file write is permitted for an agent.
    /// Returns true if the file is unowned or owned by this agent.
    pub fn can_write(&self, agent_id: &AgentId, file: &Path) -> bool {
        match self.locks.get(file) {
            Some(owner) => owner == agent_id,
            None => true,
        }
    }
}

/// State of a worker during its lifecycle.
#[derive(Debug, Clone)]
pub enum WorkerState {
    Spawning,
    Initializing,
    Working,
    Reporting,
    Completed(AgentOutcome),
    Failed(String),
}

/// Handle to a running worker, held by the orchestrator.
pub struct WorkerHandle {
    pub id: AgentId,
    pub profile: String,
    pub work_package: WorkPackage,
    pub state: WorkerState,
    pub join_handle: JoinHandle<Result<AgentOutcome>>,
    pub cancel_token: CancellationToken,
    pub spawned_at: DateTime<Utc>,
    pub last_check_in: DateTime<Utc>,
}

impl std::fmt::Debug for WorkerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerHandle")
            .field("id", &self.id)
            .field("profile", &self.profile)
            .field("state", &self.state)
            .field("spawned_at", &self.spawned_at)
            .field("last_check_in", &self.last_check_in)
            .finish()
    }
}

/// Input for task grouping: a ready task with its file scope metadata.
#[derive(Debug, Clone)]
pub struct TaskForGrouping {
    pub task_id: String,
    pub file_scope: Vec<PathBuf>,
    pub profile: String,
    pub priority: Priority,
    pub depends_on: Vec<String>,
}

/// Generate a work package ID: `wp-{8 hex chars from uuid}`.
pub fn generate_work_package_id() -> String {
    format!("wp-{}", &uuid::Uuid::new_v4().simple().to_string()[..8])
}

/// Group ready tasks into work packages based on file overlap and dependencies.
///
/// Grouping rules (from architecture):
/// 1. Tasks that modify the same files -> same work package
/// 2. Tasks with sequential dependencies -> same work package
/// 3. Independent tasks with separate file scopes -> separate work packages
pub fn group_tasks_into_packages(tasks: Vec<TaskForGrouping>) -> Vec<WorkPackage> {
    if tasks.is_empty() {
        return vec![];
    }

    let n = tasks.len();
    // Union-find: parent[i] = parent index
    let mut parent: Vec<usize> = (0..n).collect();

    fn find(parent: &mut [usize], i: usize) -> usize {
        if parent[i] != i {
            parent[i] = find(parent, parent[i]);
        }
        parent[i]
    }

    fn union(parent: &mut [usize], a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent[ra] = rb;
        }
    }

    // Build a file -> task indices map for overlap detection
    let mut file_to_tasks: HashMap<PathBuf, Vec<usize>> = HashMap::new();
    for (i, task) in tasks.iter().enumerate() {
        for file in &task.file_scope {
            file_to_tasks.entry(file.clone()).or_default().push(i);
        }
    }

    // Merge tasks that share files
    for indices in file_to_tasks.values() {
        for window in indices.windows(2) {
            union(&mut parent, window[0], window[1]);
        }
    }

    // Build task_id -> index map for dependency lookup
    let task_id_to_idx: HashMap<&str, usize> = tasks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.task_id.as_str(), i))
        .collect();

    // Merge tasks with dependencies
    for (i, task) in tasks.iter().enumerate() {
        for dep_id in &task.depends_on {
            if let Some(&dep_idx) = task_id_to_idx.get(dep_id.as_str()) {
                union(&mut parent, i, dep_idx);
            }
        }
    }

    // Group tasks by root
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let root = find(&mut parent, i);
        groups.entry(root).or_default().push(i);
    }

    // Build work packages from groups
    groups
        .into_values()
        .map(|indices| {
            let mut task_ids = Vec::new();
            let mut file_scope_set: HashMap<PathBuf, ()> = HashMap::new();
            let mut highest_priority = Priority::Low;
            let mut profile_counts: HashMap<&str, usize> = HashMap::new();

            for &idx in &indices {
                let task = &tasks[idx];
                task_ids.push(task.task_id.clone());
                for file in &task.file_scope {
                    file_scope_set.insert(file.clone(), ());
                }
                // Track highest priority
                if priority_rank(&task.priority) > priority_rank(&highest_priority) {
                    highest_priority = task.priority;
                }
                *profile_counts.entry(&task.profile).or_default() += 1;
            }

            let file_scope: Vec<PathBuf> = file_scope_set.into_keys().collect();
            let estimated_complexity = if file_scope.len() <= 2 {
                Complexity::Small
            } else if file_scope.len() <= 6 {
                Complexity::Medium
            } else {
                Complexity::Large
            };

            // Most common profile
            let profile = profile_counts
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(p, _)| p.to_string())
                .unwrap_or_else(|| "coder".to_string());

            WorkPackage {
                id: generate_work_package_id(),
                task_ids,
                file_scope,
                profile,
                priority: highest_priority,
                estimated_complexity,
            }
        })
        .collect()
}

fn priority_rank(p: &Priority) -> u8 {
    match p {
        Priority::Low => 0,
        Priority::Medium => 1,
        Priority::High => 2,
        Priority::Critical => 3,
    }
}
