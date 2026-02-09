use rustagent::agent::profile::AgentProfile;
use rustagent::security::SecurityScope;

#[test]
fn test_security_scope_default_is_permissive() {
    // P1d.AC1.2: Default SecurityScope should be permissive
    let scope = SecurityScope::default();

    assert_eq!(scope.allowed_paths, vec!["*"]);
    assert!(scope.denied_paths.is_empty());
    assert_eq!(scope.allowed_commands, vec!["*"]);
    assert!(!scope.read_only);
    assert!(scope.can_create_files);
    assert!(!scope.network_access);
}

#[test]
fn test_security_scope_deserialize() {
    // P1d.AC1.2: SecurityScope can be deserialized from TOML
    let toml_str = r#"
        allowed_paths = ["/home/user/project"]
        denied_paths = ["/etc"]
        allowed_commands = ["ls", "cat"]
        read_only = true
        can_create_files = false
        network_access = false
    "#;

    let scope: SecurityScope = toml::from_str(toml_str).expect("Failed to deserialize");

    assert_eq!(scope.allowed_paths, vec!["/home/user/project"]);
    assert_eq!(scope.denied_paths, vec!["/etc"]);
    assert_eq!(scope.allowed_commands, vec!["ls", "cat"]);
    assert!(scope.read_only);
    assert!(!scope.can_create_files);
    assert!(!scope.network_access);
}

#[test]
fn test_agent_profile_deserialize() {
    // P1d.AC3.1: AgentProfile deserializes from TOML
    let toml_str = r#"
        name = "coder"
        role = "Implementation specialist"
        system_prompt = "You are a code implementation specialist"
        allowed_tools = ["file", "shell"]
        turn_limit = 50
        token_budget = 100000

        [security]
        allowed_paths = ["/project"]
        denied_paths = []
        allowed_commands = ["ls", "cat"]
        read_only = false
        can_create_files = true
        network_access = false

        [llm]
        model = "claude-3-sonnet-20250219"
        temperature = 0.7
        max_tokens = 4096
    "#;

    let profile: AgentProfile = toml::from_str(toml_str).expect("Failed to deserialize");

    assert_eq!(profile.name, "coder");
    assert_eq!(profile.role, "Implementation specialist");
    assert_eq!(profile.system_prompt, "You are a code implementation specialist");
    assert_eq!(profile.allowed_tools, vec!["file", "shell"]);
    assert_eq!(profile.turn_limit, Some(50));
    assert_eq!(profile.token_budget, Some(100000));
    assert_eq!(profile.security.allowed_paths, vec!["/project"]);
    assert_eq!(profile.security.allowed_commands, vec!["ls", "cat"]);
    assert_eq!(profile.llm.model, Some("claude-3-sonnet-20250219".to_string()));
    assert_eq!(profile.llm.temperature, Some(0.7));
    assert_eq!(profile.llm.max_tokens, Some(4096));
}

#[test]
fn test_agent_profile_inheritance_scalar_fields() {
    // P1d.AC3.5: Scalar fields - child wins if non-empty
    let mut child = AgentProfile {
        name: "child".to_string(),
        extends: Some("parent".to_string()),
        role: "Child role".to_string(),
        system_prompt: "child prompt".to_string(),
        allowed_tools: vec![],
        security: SecurityScope::default(),
        llm: Default::default(),
        turn_limit: None,
        token_budget: None,
    };

    let parent = AgentProfile {
        name: "parent".to_string(),
        extends: None,
        role: "Parent role".to_string(),
        system_prompt: "parent prompt".to_string(),
        allowed_tools: vec![],
        security: SecurityScope::default(),
        llm: Default::default(),
        turn_limit: None,
        token_budget: None,
    };

    child.apply_inheritance(&parent);

    assert_eq!(child.role, "Child role");
    assert!(child.system_prompt.contains("parent prompt"));
    assert!(child.system_prompt.contains("child prompt"));
}

#[test]
fn test_agent_profile_inheritance_scalar_fields_empty() {
    // P1d.AC3.5: Scalar fields - parent used if child empty
    let mut child = AgentProfile {
        name: "child".to_string(),
        extends: Some("parent".to_string()),
        role: "".to_string(),
        system_prompt: "child prompt".to_string(),
        allowed_tools: vec![],
        security: SecurityScope::default(),
        llm: Default::default(),
        turn_limit: None,
        token_budget: None,
    };

    let parent = AgentProfile {
        name: "parent".to_string(),
        extends: None,
        role: "Parent role".to_string(),
        system_prompt: "parent prompt".to_string(),
        allowed_tools: vec![],
        security: SecurityScope::default(),
        llm: Default::default(),
        turn_limit: None,
        token_budget: None,
    };

    child.apply_inheritance(&parent);

    assert_eq!(child.role, "Parent role");
}

#[test]
fn test_agent_profile_inheritance_list_fields() {
    // P1d.AC3.5: List fields - child replaces parent entirely (not merged)
    let mut child = AgentProfile {
        name: "child".to_string(),
        extends: Some("parent".to_string()),
        role: "Child".to_string(),
        system_prompt: "".to_string(),
        allowed_tools: vec!["file".to_string()],
        security: SecurityScope::default(),
        llm: Default::default(),
        turn_limit: None,
        token_budget: None,
    };

    let parent = AgentProfile {
        name: "parent".to_string(),
        extends: None,
        role: "Parent".to_string(),
        system_prompt: "".to_string(),
        allowed_tools: vec!["file".to_string(), "shell".to_string()],
        security: SecurityScope::default(),
        llm: Default::default(),
        turn_limit: None,
        token_budget: None,
    };

    child.apply_inheritance(&parent);

    // Child has ["file"], so it should not inherit parent's ["file", "shell"]
    assert_eq!(child.allowed_tools, vec!["file"]);
}

#[test]
fn test_agent_profile_inheritance_list_fields_empty() {
    // P1d.AC3.5: List fields - parent used if child empty
    let mut child = AgentProfile {
        name: "child".to_string(),
        extends: Some("parent".to_string()),
        role: "Child".to_string(),
        system_prompt: "".to_string(),
        allowed_tools: vec![],
        security: SecurityScope::default(),
        llm: Default::default(),
        turn_limit: None,
        token_budget: None,
    };

    let parent = AgentProfile {
        name: "parent".to_string(),
        extends: None,
        role: "Parent".to_string(),
        system_prompt: "".to_string(),
        allowed_tools: vec!["file".to_string(), "shell".to_string()],
        security: SecurityScope::default(),
        llm: Default::default(),
        turn_limit: None,
        token_budget: None,
    };

    child.apply_inheritance(&parent);

    assert_eq!(child.allowed_tools, vec!["file", "shell"]);
}

#[test]
fn test_agent_profile_inheritance_system_prompt_appends() {
    // P1d.AC3.5: system_prompt appends with separator
    let mut child = AgentProfile {
        name: "child".to_string(),
        extends: Some("parent".to_string()),
        role: "Child".to_string(),
        system_prompt: "Child instructions".to_string(),
        allowed_tools: vec![],
        security: SecurityScope::default(),
        llm: Default::default(),
        turn_limit: None,
        token_budget: None,
    };

    let parent = AgentProfile {
        name: "parent".to_string(),
        extends: None,
        role: "Parent".to_string(),
        system_prompt: "Parent instructions".to_string(),
        allowed_tools: vec![],
        security: SecurityScope::default(),
        llm: Default::default(),
        turn_limit: None,
        token_budget: None,
    };

    child.apply_inheritance(&parent);

    assert!(child.system_prompt.contains("Parent instructions"));
    assert!(child.system_prompt.contains("Project-Specific Instructions"));
    assert!(child.system_prompt.contains("Child instructions"));
}

#[test]
fn test_agent_profile_inheritance_optional_fields() {
    // P1d.AC3.5: Optional fields - child Some wins, falls to parent if None
    let mut child = AgentProfile {
        name: "child".to_string(),
        extends: Some("parent".to_string()),
        role: "Child".to_string(),
        system_prompt: "".to_string(),
        allowed_tools: vec![],
        security: SecurityScope::default(),
        llm: Default::default(),
        turn_limit: Some(75),
        token_budget: None,
    };

    let parent = AgentProfile {
        name: "parent".to_string(),
        extends: None,
        role: "Parent".to_string(),
        system_prompt: "".to_string(),
        allowed_tools: vec![],
        security: SecurityScope::default(),
        llm: Default::default(),
        turn_limit: Some(50),
        token_budget: Some(200000),
    };

    child.apply_inheritance(&parent);

    assert_eq!(child.turn_limit, Some(75)); // child wins
    assert_eq!(child.token_budget, Some(200000)); // from parent
}

#[test]
fn test_agent_profile_inheritance_llm_config() {
    // P1d.AC3.5: LLM config - child Some wins, falls to parent if None
    let mut child = AgentProfile {
        name: "child".to_string(),
        extends: Some("parent".to_string()),
        role: "Child".to_string(),
        system_prompt: "".to_string(),
        allowed_tools: vec![],
        security: SecurityScope::default(),
        llm: rustagent::agent::profile::ProfileLlmConfig {
            model: Some("child-model".to_string()),
            temperature: None,
            max_tokens: None,
        },
        turn_limit: None,
        token_budget: None,
    };

    let parent = AgentProfile {
        name: "parent".to_string(),
        extends: None,
        role: "Parent".to_string(),
        system_prompt: "".to_string(),
        allowed_tools: vec![],
        security: SecurityScope::default(),
        llm: rustagent::agent::profile::ProfileLlmConfig {
            model: Some("parent-model".to_string()),
            temperature: Some(0.5),
            max_tokens: Some(2000),
        },
        turn_limit: None,
        token_budget: None,
    };

    child.apply_inheritance(&parent);

    assert_eq!(child.llm.model, Some("child-model".to_string())); // child wins
    assert_eq!(child.llm.temperature, Some(0.5)); // from parent
    assert_eq!(child.llm.max_tokens, Some(2000)); // from parent
}
