use chrono::Utc;
use rustagent::config::Config;
use rustagent::ralph::RalphLoop;
use rustagent::spec::{Spec, Task, TaskStatus};
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_ralph_loop_creation() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("config.toml");
    let spec_path = temp.path().join("test.json");

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

    let spec = Spec {
        name: "test".to_string(),
        description: "Test".to_string(),
        branch_name: "feature/test".to_string(),
        created_at: Utc::now(),
        tasks: vec![],
        learnings: vec![],
    };
    spec.save(&spec_path).unwrap();

    let config = Config::load(&config_path).unwrap();
    let ralph = RalphLoop::new(config, spec_path.to_str().unwrap().to_string(), None).unwrap();

    assert!(ralph.spec_path.ends_with("test.json"));
}

#[test]
fn test_find_next_pending_task() {
    let spec = Spec {
        name: "test".to_string(),
        description: "Test".to_string(),
        branch_name: "feature/test".to_string(),
        created_at: Utc::now(),
        tasks: vec![
            Task {
                id: "task-1".to_string(),
                title: "Task 1".to_string(),
                description: "First task".to_string(),
                acceptance_criteria: vec![],
                status: TaskStatus::Complete,
                blocked_reason: None,
                completed_at: Some(Utc::now()),
            },
            Task {
                id: "task-2".to_string(),
                title: "Task 2".to_string(),
                description: "Second task".to_string(),
                acceptance_criteria: vec![],
                status: TaskStatus::Pending,
                blocked_reason: None,
                completed_at: None,
            },
        ],
        learnings: vec![],
    };

    let next = spec.find_next_task();
    assert!(next.is_some());
    assert_eq!(next.unwrap().id, "task-2");
}
