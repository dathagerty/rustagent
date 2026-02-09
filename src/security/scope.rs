use serde::{Deserialize, Serialize};

/// Security scope for an agent, controlling what it can access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityScope {
    /// Paths the agent is allowed to access
    pub allowed_paths: Vec<String>,

    /// Paths the agent is explicitly denied access to
    pub denied_paths: Vec<String>,

    /// Shell commands the agent is allowed to run
    pub allowed_commands: Vec<String>,

    /// Whether the agent has read-only access
    pub read_only: bool,

    /// Whether the agent can create new files
    pub can_create_files: bool,

    /// Whether the agent can access network resources
    pub network_access: bool,
}

impl Default for SecurityScope {
    fn default() -> Self {
        Self {
            allowed_paths: vec!["*".to_string()],
            denied_paths: vec![],
            allowed_commands: vec!["*".to_string()],
            read_only: false,
            can_create_files: true,
            network_access: false,
        }
    }
}
