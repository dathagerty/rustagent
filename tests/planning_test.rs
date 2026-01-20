use rustagent::planning::PlanningAgent;
use rustagent::config::Config;
use tempfile::TempDir;
use std::fs;

#[tokio::test]
async fn test_planning_agent_creation() {
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
    let agent = PlanningAgent::new(config, temp.path().to_str().unwrap().to_string());

    assert!(agent.spec_dir.ends_with(temp.path().to_str().unwrap()));
}
