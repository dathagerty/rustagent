use rustagent::config::{Config, LlmProvider};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_config_from_toml() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("config.toml");

    fs::write(&config_path, r#"
[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"

[anthropic]
api_key = "test-key"

[rustagent]
spec_dir = "specs"
"#).unwrap();

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

    fs::write(&config_path, r#"
[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"

[anthropic]
api_key = "${TEST_API_KEY}"

[rustagent]
spec_dir = "specs"
"#).unwrap();

    let config = Config::load(&config_path).unwrap();
    assert_eq!(config.anthropic.as_ref().unwrap().api_key, "secret123");

    unsafe {
        std::env::remove_var("TEST_API_KEY");
    }
}
