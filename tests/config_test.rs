use rustagent::config::{Config, LlmProvider, ShellPolicy};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_config_from_toml() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("config.toml");

    fs::write(
        &config_path,
        r#"
[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"

[anthropic]
api_key = "test-key"

[rustagent]
spec_dir = "specs"
"#,
    )
    .unwrap();

    let config = Config::load(&config_path).unwrap();
    assert_eq!(config.llm.provider, LlmProvider::Anthropic);
    assert_eq!(config.llm.model, "claude-sonnet-4-20250514");
}

#[test]
fn test_config_env_var_substitution() {
    unsafe {
        std::env::set_var("TEST_API_KEY", "secret123");
    }

    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("config.toml");

    fs::write(
        &config_path,
        r#"
[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"

[anthropic]
api_key = "${TEST_API_KEY}"

[rustagent]
spec_dir = "specs"
"#,
    )
    .unwrap();

    let config = Config::load(&config_path).unwrap();
    assert_eq!(config.anthropic.as_ref().unwrap().api_key, "secret123");

    unsafe {
        std::env::remove_var("TEST_API_KEY");
    }
}

#[test]
fn test_security_config_defaults() {
    let config_str = r#"
[llm]
provider = "anthropic"
model = "claude-sonnet-4"

[anthropic]
api_key = "test-key"

[rustagent]
spec_dir = "specs"
"#;

    let config: Config = toml::from_str(config_str).unwrap();

    // Should have default security config
    assert_eq!(config.security.shell_policy, ShellPolicy::Allowlist);
    assert!(config.security.allowed_commands.contains(&"git".to_string()));
    assert_eq!(config.security.max_file_size_mb, 10);
    assert_eq!(config.security.allowed_paths, vec![".".to_string()]);
}

#[test]
fn test_security_config_custom() {
    let config_str = r#"
[llm]
provider = "anthropic"
model = "claude-sonnet-4"

[anthropic]
api_key = "test-key"

[rustagent]
spec_dir = "specs"

[security]
shell_policy = "blocklist"
allowed_commands = ["git", "cargo"]
blocked_patterns = ["rm -rf", "eval"]
max_file_size_mb = 50
allowed_paths = [".", "/tmp"]
"#;

    let config: Config = toml::from_str(config_str).unwrap();

    assert_eq!(config.security.shell_policy, ShellPolicy::Blocklist);
    assert_eq!(config.security.allowed_commands.len(), 2);
    assert_eq!(config.security.blocked_patterns.len(), 2);
    assert_eq!(config.security.max_file_size_mb, 50);
    assert_eq!(config.security.allowed_paths.len(), 2);
}
