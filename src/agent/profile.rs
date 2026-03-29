use crate::security::SecurityScope;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// LLM configuration for an agent profile
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileLlmConfig {
    /// Model name to use (e.g., "claude-3-sonnet-20250219")
    pub model: Option<String>,

    /// Temperature for sampling (0.0 to 1.0+)
    pub temperature: Option<f64>,

    /// Maximum tokens to generate
    pub max_tokens: Option<usize>,
}

/// Agent profile describing behavior and capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    /// Name of the profile (e.g., "coder", "reviewer")
    pub name: String,

    /// Optional parent profile to inherit from
    pub extends: Option<String>,

    /// Role description (e.g., "Implementation specialist")
    pub role: String,

    /// System prompt to guide behavior
    pub system_prompt: String,

    /// List of tools the agent is allowed to use
    pub allowed_tools: Vec<String>,

    /// Security configuration for this profile
    pub security: SecurityScope,

    /// LLM configuration overrides
    #[serde(default)]
    pub llm: ProfileLlmConfig,

    /// Maximum turns before stopping (None = no limit)
    pub turn_limit: Option<usize>,

    /// Token budget for this run (None = no limit)
    pub token_budget: Option<usize>,
}

impl AgentProfile {
    /// Apply inheritance from a parent profile
    ///
    /// Rules:
    /// - Scalar fields: only override if self has a meaningful value
    /// - List fields: child replaces parent entirely (not merged)
    /// - system_prompt: child appended to parent with separator
    /// - Optional fields: Some in child wins, falls through to parent if None
    pub fn apply_inheritance(&mut self, parent: &AgentProfile) {
        // Scalar fields: child wins only if non-empty
        if self.role.is_empty() {
            self.role.clone_from(&parent.role);
        }

        // system_prompt: append child to parent
        if !self.system_prompt.is_empty() && !parent.system_prompt.is_empty() {
            self.system_prompt = format!(
                "{}\n\n## Project-Specific Instructions\n{}",
                parent.system_prompt, self.system_prompt
            );
        } else if self.system_prompt.is_empty() {
            self.system_prompt.clone_from(&parent.system_prompt);
        }

        // List fields: child replaces parent entirely
        if self.allowed_tools.is_empty() {
            self.allowed_tools.clone_from(&parent.allowed_tools);
        }

        // Security: take from parent if not set in child
        // Simple heuristic: if child has default values, use parent's
        if self.security.allowed_paths == vec!["*"] && parent.security.allowed_paths != vec!["*"] {
            self.security
                .allowed_paths
                .clone_from(&parent.security.allowed_paths);
        }
        if self.security.denied_paths.is_empty() && !parent.security.denied_paths.is_empty() {
            self.security
                .denied_paths
                .clone_from(&parent.security.denied_paths);
        }
        if self.security.allowed_commands == vec!["*"]
            && parent.security.allowed_commands != vec!["*"]
        {
            self.security
                .allowed_commands
                .clone_from(&parent.security.allowed_commands);
        }

        // LLM config: child Some wins, falls through to parent if None
        if self.llm.model.is_none() {
            self.llm.model.clone_from(&parent.llm.model);
        }
        if self.llm.temperature.is_none() {
            self.llm.temperature = parent.llm.temperature;
        }
        if self.llm.max_tokens.is_none() {
            self.llm.max_tokens = parent.llm.max_tokens;
        }

        // Optional numeric fields: child Some wins, falls through to parent if None
        if self.turn_limit.is_none() {
            self.turn_limit = parent.turn_limit;
        }
        if self.token_budget.is_none() {
            self.token_budget = parent.token_budget;
        }
    }
}

/// Resolve a profile by name, checking in order: project-level, user-level, built-in.
///
/// Supports inheritance via `extends` field. Returns error on cycles or unknown profiles.
pub fn resolve_profile(name: &str, project_path: Option<&Path>) -> Result<AgentProfile> {
    let mut visited = HashSet::new();
    resolve_profile_impl(name, project_path, &mut visited)
}

fn resolve_profile_impl(
    name: &str,
    project_path: Option<&Path>,
    visited: &mut HashSet<String>,
) -> Result<AgentProfile> {
    // Check for cycles in inheritance
    if visited.contains(name) {
        bail!(
            "inheritance cycle detected: profile '{}' extends itself",
            name
        );
    }
    visited.insert(name.to_string());

    // 1. Project-level: .rustagent/profiles/{name}.toml
    if let Some(path) = project_path {
        let profile_path = path
            .join(".rustagent/profiles")
            .join(format!("{}.toml", name));
        if profile_path.exists() {
            let content = std::fs::read_to_string(&profile_path)?;
            let mut profile: AgentProfile = toml::from_str(&content)?;
            if let Some(parent_name) = &profile.extends.clone() {
                let parent = resolve_profile_impl(parent_name, project_path, visited)?;
                profile.apply_inheritance(&parent);
            }
            return Ok(profile);
        }
    }

    // 2. User-level: ~/.config/rustagent/profiles/{name}.toml
    if let Some(config_dir) = dirs::config_dir() {
        let profile_path = config_dir
            .join("rustagent/profiles")
            .join(format!("{}.toml", name));
        if profile_path.exists() {
            let content = std::fs::read_to_string(&profile_path)?;
            let mut profile: AgentProfile = toml::from_str(&content)?;
            if let Some(parent_name) = &profile.extends.clone() {
                let parent = resolve_profile_impl(parent_name, project_path, visited)?;
                profile.apply_inheritance(&parent);
            }
            return Ok(profile);
        }
    }

    // 3. Built-in profiles
    match name {
        "planner" => Ok(crate::agent::builtin_profiles::planner()),
        "coder" => Ok(crate::agent::builtin_profiles::coder()),
        "reviewer" => Ok(crate::agent::builtin_profiles::reviewer()),
        "tester" => Ok(crate::agent::builtin_profiles::tester()),
        "researcher" => Ok(crate::agent::builtin_profiles::researcher()),
        _ => bail!("Unknown profile: {}", name),
    }
}
