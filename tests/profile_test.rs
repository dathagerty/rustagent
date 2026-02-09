use rustagent::agent::profile::{AgentProfile, resolve_profile};
use rustagent::security::SecurityScope;
use std::fs;
use tempfile::TempDir;

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
    assert_eq!(
        profile.system_prompt,
        "You are a code implementation specialist"
    );
    assert_eq!(profile.allowed_tools, vec!["file", "shell"]);
    assert_eq!(profile.turn_limit, Some(50));
    assert_eq!(profile.token_budget, Some(100000));
    assert_eq!(profile.security.allowed_paths, vec!["/project"]);
    assert_eq!(profile.security.allowed_commands, vec!["ls", "cat"]);
    assert_eq!(
        profile.llm.model,
        Some("claude-3-sonnet-20250219".to_string())
    );
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
    assert!(
        child
            .system_prompt
            .contains("Project-Specific Instructions")
    );
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

// Built-in profiles tests

#[test]
fn test_resolve_builtin_coder_profile() {
    // P1d.AC3.2: resolve_profile("coder", None) returns built-in coder profile
    let profile = resolve_profile("coder", None).expect("Failed to resolve coder profile");

    assert_eq!(profile.name, "coder");
    assert_eq!(profile.role, "Implementation specialist");
    assert!(profile.system_prompt.len() > 0);
    assert!(profile.allowed_tools.contains(&"file".to_string()));
    assert!(profile.allowed_tools.contains(&"shell".to_string()));
}

#[test]
fn test_resolve_builtin_planner_profile() {
    // P1d.AC3.2: resolve_profile("planner", None) returns built-in planner profile
    let profile = resolve_profile("planner", None).expect("Failed to resolve planner profile");

    assert_eq!(profile.name, "planner");
    assert_eq!(profile.role, "Task breakdown specialist");
    assert!(profile.system_prompt.len() > 0);
}

#[test]
fn test_resolve_builtin_reviewer_profile() {
    // P1d.AC3.2: resolve_profile("reviewer", None) returns built-in reviewer profile
    let profile = resolve_profile("reviewer", None).expect("Failed to resolve reviewer profile");

    assert_eq!(profile.name, "reviewer");
    assert_eq!(profile.role, "Code review specialist");
    assert!(profile.system_prompt.len() > 0);
}

#[test]
fn test_resolve_builtin_tester_profile() {
    // P1d.AC3.2: resolve_profile("tester", None) returns built-in tester profile
    let profile = resolve_profile("tester", None).expect("Failed to resolve tester profile");

    assert_eq!(profile.name, "tester");
    assert_eq!(profile.role, "Test implementation specialist");
    assert!(profile.system_prompt.len() > 0);
}

#[test]
fn test_resolve_builtin_researcher_profile() {
    // P1d.AC3.2: resolve_profile("researcher", None) returns built-in researcher profile
    let profile = resolve_profile("researcher", None).expect("Failed to resolve researcher profile");

    assert_eq!(profile.name, "researcher");
    assert_eq!(profile.role, "Information gathering specialist");
    assert!(profile.system_prompt.len() > 0);
}

#[test]
fn test_resolve_unknown_profile_fails() {
    // Unknown profile should fail
    let result = resolve_profile("nonexistent_profile", None);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Unknown profile"));
}

#[test]
fn test_resolve_project_level_profile() {
    // P1d.AC3.3: Create a tempdir with .rustagent/profiles/custom.toml
    let tempdir = TempDir::new().expect("Failed to create tempdir");
    let project_path = tempdir.path();

    // Create .rustagent/profiles directory
    let profiles_dir = project_path.join(".rustagent").join("profiles");
    fs::create_dir_all(&profiles_dir).expect("Failed to create profiles directory");

    // Create custom.toml
    let custom_toml = r#"
name = "custom"
role = "Custom role"
system_prompt = "Custom system prompt"
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

    let profile_path = profiles_dir.join("custom.toml");
    fs::write(&profile_path, custom_toml).expect("Failed to write custom.toml");

    let profile =
        resolve_profile("custom", Some(project_path)).expect("Failed to resolve custom profile");

    assert_eq!(profile.name, "custom");
    assert_eq!(profile.role, "Custom role");
}

#[test]
fn test_resolve_project_level_overrides_builtin() {
    // P1d.AC3.4: Project-level "coder" profile should override built-in
    let tempdir = TempDir::new().expect("Failed to create tempdir");
    let project_path = tempdir.path();

    // Create .rustagent/profiles directory
    let profiles_dir = project_path.join(".rustagent").join("profiles");
    fs::create_dir_all(&profiles_dir).expect("Failed to create profiles directory");

    // Create project-level coder.toml
    let project_coder = r#"
name = "coder"
role = "Project-specific coder"
system_prompt = "Project-specific system prompt"
allowed_tools = ["file", "shell"]

[security]
allowed_paths = ["/project"]
denied_paths = []
allowed_commands = ["*"]
read_only = false
can_create_files = true
network_access = false
"#;

    let profile_path = profiles_dir.join("coder.toml");
    fs::write(&profile_path, project_coder).expect("Failed to write coder.toml");

    let profile = resolve_profile("coder", Some(project_path))
        .expect("Failed to resolve coder profile");

    assert_eq!(profile.role, "Project-specific coder");
}

#[test]
fn test_resolve_profile_with_inheritance() {
    // P1d.AC3.5: Custom profile extends built-in, inheritance applied
    let tempdir = TempDir::new().expect("Failed to create tempdir");
    let project_path = tempdir.path();

    // Create .rustagent/profiles directory
    let profiles_dir = project_path.join(".rustagent").join("profiles");
    fs::create_dir_all(&profiles_dir).expect("Failed to create profiles directory");

    // Create custom.toml that extends built-in "coder"
    let custom_toml = r#"
name = "custom"
extends = "coder"
role = ""
system_prompt = "Custom project instructions"
allowed_tools = []

[security]
allowed_paths = ["*"]
denied_paths = []
allowed_commands = ["*"]
read_only = false
can_create_files = true
network_access = false

[llm]
"#;

    let profile_path = profiles_dir.join("custom.toml");
    fs::write(&profile_path, custom_toml).expect("Failed to write custom.toml");

    let profile = resolve_profile("custom", Some(project_path))
        .expect("Failed to resolve custom profile");

    // Should inherit role from coder (since custom is empty)
    assert_eq!(profile.role, "Implementation specialist");
    // Should have coder's tools (since custom is empty)
    assert!(profile.allowed_tools.contains(&"file".to_string()));
    assert!(profile.allowed_tools.contains(&"shell".to_string()));
    // Should have combined system_prompt
    assert!(profile.system_prompt.contains("Custom project instructions"));
}

#[test]
fn test_resolve_profile_cycle_detection() {
    // P1d.AC3.5: Cycle detection in inheritance chain
    let tempdir = TempDir::new().expect("Failed to create tempdir");
    let project_path = tempdir.path();

    // Create .rustagent/profiles directory
    let profiles_dir = project_path.join(".rustagent").join("profiles");
    fs::create_dir_all(&profiles_dir).expect("Failed to create profiles directory");

    // Create a.toml that extends b
    let a_toml = r#"
name = "a"
extends = "b"
role = "A"
system_prompt = ""
allowed_tools = []

[security]
allowed_paths = ["*"]
denied_paths = []
allowed_commands = ["*"]
read_only = false
can_create_files = true
network_access = false
"#;

    // Create b.toml that extends a (cycle!)
    let b_toml = r#"
name = "b"
extends = "a"
role = "B"
system_prompt = ""
allowed_tools = []

[security]
allowed_paths = ["*"]
denied_paths = []
allowed_commands = ["*"]
read_only = false
can_create_files = true
network_access = false
"#;

    fs::write(profiles_dir.join("a.toml"), a_toml).expect("Failed to write a.toml");
    fs::write(profiles_dir.join("b.toml"), b_toml).expect("Failed to write b.toml");

    let result = resolve_profile("a", Some(project_path));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cycle"));
}
