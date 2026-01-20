use rustagent::config::{Config, LlmProvider, LlmConfig, AnthropicConfig, RustagentConfig};
use rustagent::spec::{Spec, Task, TaskStatus};
use tempfile::TempDir;

#[test]
fn test_full_workflow() {
    let temp = TempDir::new().unwrap();

    // Create config
    let config = Config {
        llm: LlmConfig {
            provider: LlmProvider::Anthropic,
            model: "claude-sonnet-4-20250514".to_string(),
        },
        anthropic: Some(AnthropicConfig {
            api_key: "test-key".to_string(),
        }),
        openai: None,
        ollama: None,
        rustagent: RustagentConfig {
            spec_dir: "specs".to_string(),
            max_iterations: Some(10),
        },
    };

    // Create spec
    let spec = Spec {
        name: "test-feature".to_string(),
        description: "A test feature".to_string(),
        branch_name: "feature/test".to_string(),
        created_at: "2026-01-19T12:00:00Z".to_string(),
        tasks: vec![
            Task {
                id: "task-1".to_string(),
                title: "First task".to_string(),
                description: "Do the first thing".to_string(),
                acceptance_criteria: vec!["Must work".to_string()],
                status: TaskStatus::Pending,
                blocked_reason: None,
                completed_at: None,
            },
        ],
        learnings: vec![],
    };

    let spec_path = temp.path().join("test.json");
    spec.save(&spec_path).unwrap();

    // Verify spec was saved
    let loaded_spec = Spec::load(&spec_path).unwrap();
    assert_eq!(loaded_spec.name, "test-feature");
    assert_eq!(loaded_spec.tasks.len(), 1);

    // Verify config structure
    assert_eq!(config.llm.provider, LlmProvider::Anthropic);
    assert!(config.anthropic.is_some());
}
