use rustagent::spec::{Spec, Task, TaskStatus};
use tempfile::TempDir;

#[test]
fn test_spec_serialization() {
    let spec = Spec {
        name: "test-feature".to_string(),
        description: "A test feature".to_string(),
        branch_name: "feature/test".to_string(),
        created_at: "2026-01-19T12:00:00Z".to_string(),
        tasks: vec![
            Task {
                id: "task-1".to_string(),
                title: "Implement X".to_string(),
                description: "Description".to_string(),
                acceptance_criteria: vec!["Criterion 1".to_string()],
                status: TaskStatus::Pending,
                blocked_reason: None,
                completed_at: None,
            }
        ],
        learnings: vec![],
    };

    let json = serde_json::to_string_pretty(&spec).unwrap();
    assert!(json.contains("test-feature"));
    assert!(json.contains("pending"));
}

#[test]
fn test_spec_save_and_load() {
    let temp = TempDir::new().unwrap();
    let spec_path = temp.path().join("test.json");

    let spec = Spec {
        name: "test".to_string(),
        description: "Test".to_string(),
        branch_name: "feature/test".to_string(),
        created_at: "2026-01-19T12:00:00Z".to_string(),
        tasks: vec![],
        learnings: vec![],
    };

    spec.save(&spec_path).unwrap();
    let loaded = Spec::load(&spec_path).unwrap();

    assert_eq!(loaded.name, "test");
}
