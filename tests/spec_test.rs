use chrono::Utc;
use rustagent::spec::{Spec, Task, TaskStatus};
use tempfile::TempDir;

#[test]
fn test_spec_serialization() {
    let spec = Spec {
        name: "test-feature".to_string(),
        description: "A test feature".to_string(),
        branch_name: "feature/test".to_string(),
        created_at: Utc::now(),
        tasks: vec![Task {
            id: "task-1".to_string(),
            title: "Implement X".to_string(),
            description: "Description".to_string(),
            acceptance_criteria: vec!["Criterion 1".to_string()],
            status: TaskStatus::Pending,
            blocked_reason: None,
            completed_at: None,
        }],
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
        created_at: Utc::now(),
        tasks: vec![],
        learnings: vec![],
    };

    spec.save(&spec_path).unwrap();
    let loaded = Spec::load(&spec_path).unwrap();

    assert_eq!(loaded.name, "test");
}

#[test]
fn test_spec_uses_datetime_types() {
    let now = Utc::now();
    let spec = Spec {
        name: "test".to_string(),
        description: "test spec".to_string(),
        branch_name: "feature/test".to_string(),
        created_at: now,
        tasks: vec![],
        learnings: vec![],
    };

    // Should serialize to RFC3339 format
    let json = serde_json::to_string(&spec).unwrap();
    // Chrono serializes with 'Z' suffix, which is valid RFC3339
    assert!(json.contains(&format!("{}Z", now.format("%Y-%m-%dT%H:%M:%S%.f"))));
}

#[test]
fn test_task_completion_timestamp() {
    let completed = Utc::now();
    let task = Task {
        id: "task-1".to_string(),
        title: "Test".to_string(),
        description: "desc".to_string(),
        acceptance_criteria: vec![],
        status: TaskStatus::Complete,
        blocked_reason: None,
        completed_at: Some(completed),
    };

    let json = serde_json::to_string(&task).unwrap();
    // Chrono serializes with 'Z' suffix, which is valid RFC3339
    assert!(json.contains(&format!("{}Z", completed.format("%Y-%m-%dT%H:%M:%S%.f"))));
}

#[test]
fn test_spec_save_creates_parent_directories() {
    let dir = tempfile::tempdir().unwrap();
    let nested_path = dir
        .path()
        .join("level1")
        .join("level2")
        .join("level3")
        .join("spec.json");

    let spec = Spec {
        name: "test".to_string(),
        description: "test spec".to_string(),
        branch_name: "feature/test".to_string(),
        created_at: Utc::now(),
        tasks: vec![],
        learnings: vec![],
    };

    // Should create all parent directories
    spec.save(&nested_path).unwrap();
    assert!(nested_path.exists());

    // Should be able to load it back
    let loaded = Spec::load(&nested_path).unwrap();
    assert_eq!(loaded.name, "test");
}
