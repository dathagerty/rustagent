use crate::agent::profile::{AgentProfile, ProfileLlmConfig};
use crate::security::SecurityScope;

/// Built-in "planner" profile for task breakdown and planning
pub fn planner() -> AgentProfile {
    AgentProfile {
        name: "planner".to_string(),
        extends: None,
        role: "Task breakdown specialist".to_string(),
        system_prompt: concat!(
            "Break work into tasks that can be completed independently. ",
            "Keep tasks small enough for a single focused session. ",
            "Specify acceptance criteria for every task.\n",
            "- When you make a non-trivial choice between alternatives, log a decision using log_decision.\n",
            "- When you discover something noteworthy, record it using record_observation.\n",
            "- Signal completion or blocking using the signal tool. Do not simply stop.",
        ).to_string(),
        allowed_tools: vec![
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
        system_prompt: concat!(
            "Check your work against the acceptance criteria before signaling completion. ",
            "Only modify files within your declared scope. ",
            "Commit logical units of work.\n",
            "- When you make a non-trivial choice between alternatives, log a decision using log_decision.\n",
            "- When you discover something noteworthy, record it using record_observation.\n",
            "- Signal completion or blocking using the signal tool. Do not simply stop.",
        ).to_string(),
        allowed_tools: vec![
            "file".to_string(),
            "shell".to_string(),
            "graph".to_string(),
            "agent".to_string(),
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
        system_prompt: concat!(
            "Do not modify files. Report issues as Observation nodes. ",
            "Approve or reject via the signal tool with specific feedback.\n",
            "- When you make a non-trivial choice between alternatives, log a decision using log_decision.\n",
            "- When you discover something noteworthy, record it using record_observation.\n",
            "- Signal completion or blocking using the signal tool. Do not simply stop.",
        ).to_string(),
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
        system_prompt: concat!(
            "Write tests that verify behavior, not implementation details. ",
            "Test edge cases and error conditions. ",
            "Ensure tests are clear and maintainable.\n",
            "- When you make a non-trivial choice between alternatives, log a decision using log_decision.\n",
            "- When you discover something noteworthy, record it using record_observation.\n",
            "- Signal completion or blocking using the signal tool. Do not simply stop.",
        ).to_string(),
        allowed_tools: vec![
            "file".to_string(),
            "shell".to_string(),
            "graph".to_string(),
            "agent".to_string(),
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
        system_prompt: concat!(
            "Document all findings as Observation nodes. ",
            "Provide specific file paths and line numbers. ",
            "Organize findings by relevance to the goal.\n",
            "- When you make a non-trivial choice between alternatives, log a decision using log_decision.\n",
            "- When you discover something noteworthy, record it using record_observation.\n",
            "- Signal completion or blocking using the signal tool. Do not simply stop.",
        ).to_string(),
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
