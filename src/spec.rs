use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Complete,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub acceptance_criteria: Vec<String>,
    pub status: TaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spec {
    pub name: String,
    pub description: String,
    pub branch_name: String,
    pub created_at: String,
    pub tasks: Vec<Task>,
    pub learnings: Vec<String>,
}

impl Spec {
    /// Load a spec from a JSON file
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let spec = serde_json::from_str(&content)?;
        Ok(spec)
    }

    /// Save the spec to a JSON file
    pub fn save(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Find the next task that is pending (not in progress, complete, or blocked)
    pub fn find_next_task(&self) -> Option<&Task> {
        self.tasks
            .iter()
            .find(|task| task.status == TaskStatus::Pending)
    }

    /// Find a task by ID and return a mutable reference
    pub fn find_task_mut(&mut self, task_id: &str) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|task| task.id == task_id)
    }

    /// Add a learning to the spec
    pub fn add_learning(&mut self, learning: String) {
        self.learnings.push(learning);
    }
}
