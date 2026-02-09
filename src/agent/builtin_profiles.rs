use crate::agent::profile::{AgentProfile, ProfileLlmConfig};
use crate::security::SecurityScope;

/// Built-in "planner" profile for task breakdown and planning
pub fn planner() -> AgentProfile {
    AgentProfile {
        name: "planner".to_string(),
        extends: None,
        role: "Task breakdown specialist".to_string(),
        system_prompt: "You are a task breakdown specialist. Your role is to analyze high-level goals and break them into concrete, actionable tasks. Each task should have clear acceptance criteria and be assigned to the most appropriate agent type (coder, reviewer, tester, or researcher). Prioritize tasks based on dependencies and criticality.".to_string(),
        allowed_tools: vec![
            "file".to_string(),
            "shell".to_string(),
            "graph".to_string(),
            "signal_completion".to_string(),
        ],
        security: SecurityScope {
            allowed_paths: vec!["*".to_string()],
            denied_paths: vec![],
            allowed_commands: vec!["*".to_string()],
            read_only: true,
            can_create_files: false,
            network_access: false,
        },
        llm: ProfileLlmConfig::default(),
        turn_limit: Some(100),
        token_budget: Some(200_000),
    }
}

/// Built-in "coder" profile for implementation work
pub fn coder() -> AgentProfile {
    AgentProfile {
        name: "coder".to_string(),
        extends: None,
        role: "Implementation specialist".to_string(),
        system_prompt: "You are an implementation specialist. Your role is to implement features and fix bugs by writing and modifying code. Follow the project's conventions and code style. Test your changes before marking tasks complete. Prioritize clarity and maintainability over clever solutions.".to_string(),
        allowed_tools: vec![
            "file".to_string(),
            "shell".to_string(),
            "graph".to_string(),
            "signal_completion".to_string(),
        ],
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
        token_budget: Some(300_000),
    }
}

/// Built-in "reviewer" profile for code review
pub fn reviewer() -> AgentProfile {
    AgentProfile {
        name: "reviewer".to_string(),
        extends: None,
        role: "Code review specialist".to_string(),
        system_prompt: "You are a code review specialist. Your role is to review code changes and provide constructive feedback. Check for: correctness, performance, security issues, adherence to project conventions, test coverage, and documentation. Point out both issues and good practices.".to_string(),
        allowed_tools: vec![
            "file".to_string(),
            "shell".to_string(),
            "graph".to_string(),
            "signal_completion".to_string(),
        ],
        security: SecurityScope {
            allowed_paths: vec!["*".to_string()],
            denied_paths: vec![],
            allowed_commands: vec!["*".to_string()],
            read_only: true,
            can_create_files: false,
            network_access: false,
        },
        llm: ProfileLlmConfig::default(),
        turn_limit: Some(100),
        token_budget: Some(200_000),
    }
}

/// Built-in "tester" profile for test writing and quality assurance
pub fn tester() -> AgentProfile {
    AgentProfile {
        name: "tester".to_string(),
        extends: None,
        role: "Test implementation specialist".to_string(),
        system_prompt: "You are a test implementation specialist. Your role is to write comprehensive tests including unit tests, integration tests, and edge cases. Ensure tests are clear, maintainable, and provide good coverage. Focus on testing behavior, not implementation details.".to_string(),
        allowed_tools: vec![
            "file".to_string(),
            "shell".to_string(),
            "graph".to_string(),
            "signal_completion".to_string(),
        ],
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
        token_budget: Some(250_000),
    }
}

/// Built-in "researcher" profile for information gathering and investigation
pub fn researcher() -> AgentProfile {
    AgentProfile {
        name: "researcher".to_string(),
        extends: None,
        role: "Information gathering specialist".to_string(),
        system_prompt: "You are an information gathering specialist. Your role is to investigate issues, gather requirements, explore solutions, and compile findings. Use available tools to explore the codebase, run searches, and gather context. Document your findings clearly.".to_string(),
        allowed_tools: vec![
            "file".to_string(),
            "shell".to_string(),
            "graph".to_string(),
            "signal_completion".to_string(),
        ],
        security: SecurityScope {
            allowed_paths: vec!["*".to_string()],
            denied_paths: vec![],
            allowed_commands: vec!["*".to_string()],
            read_only: true,
            can_create_files: false,
            network_access: false,
        },
        llm: ProfileLlmConfig::default(),
        turn_limit: Some(100),
        token_budget: Some(200_000),
    }
}
