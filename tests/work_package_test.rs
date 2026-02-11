use rustagent::agent::work_package::*;
use rustagent::agent::AgentOutcome;
use rustagent::graph::Priority;
use std::path::PathBuf;

/// P2b.AC1.1: WorkPackage has all required fields
#[test]
fn test_work_package_fields() {
    let wp = WorkPackage {
        id: "wp-12345678".to_string(),
        task_ids: vec!["t1".to_string(), "t2".to_string()],
        file_scope: vec![PathBuf::from("src/main.rs")],
        profile: "coder".to_string(),
        priority: Priority::High,
        estimated_complexity: Complexity::Small,
    };
    assert_eq!(wp.id, "wp-12345678");
    assert_eq!(wp.task_ids.len(), 2);
    assert_eq!(wp.file_scope.len(), 1);
    assert_eq!(wp.profile, "coder");
    assert_eq!(wp.priority, Priority::High);
    assert_eq!(wp.estimated_complexity, Complexity::Small);
}

/// P2b.AC1.2: Complexity enum has Small, Medium, Large variants
#[test]
fn test_complexity_variants() {
    let small = Complexity::Small;
    let medium = Complexity::Medium;
    let large = Complexity::Large;
    assert_ne!(small, medium);
    assert_ne!(medium, large);
    let debug = format!("{:?}", small);
    assert!(debug.contains("Small"));
}

/// P2b.AC2.1: Acquire grants ownership when no conflicts
#[test]
fn test_file_ownership_acquire_success() {
    let mut map = FileOwnershipMap::new();
    let files = vec![PathBuf::from("src/main.rs"), PathBuf::from("src/lib.rs")];
    map.acquire(&"a1".to_string(), &files).unwrap();
    assert!(map.can_write(&"a1".to_string(), &PathBuf::from("src/main.rs")));
    assert!(map.can_write(&"a1".to_string(), &PathBuf::from("src/lib.rs")));
}

/// P2b.AC2.2: Acquire returns error when file is owned by different agent
#[test]
fn test_file_ownership_acquire_conflict() {
    let mut map = FileOwnershipMap::new();
    let files = vec![PathBuf::from("src/main.rs")];
    map.acquire(&"a1".to_string(), &files).unwrap();

    let result = map.acquire(&"a2".to_string(), &files);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already owned"));
}

/// P2b.AC2.3: Release frees all files owned by an agent
#[test]
fn test_file_ownership_release() {
    let mut map = FileOwnershipMap::new();
    let files = vec![PathBuf::from("src/main.rs")];
    map.acquire(&"a1".to_string(), &files).unwrap();
    map.release(&"a1".to_string());

    // a2 can now acquire the same files
    map.acquire(&"a2".to_string(), &files).unwrap();
    assert!(map.can_write(&"a2".to_string(), &PathBuf::from("src/main.rs")));
}

/// P2b.AC2.4: can_write returns true for owned files
#[test]
fn test_can_write_owned() {
    let mut map = FileOwnershipMap::new();
    let files = vec![PathBuf::from("src/main.rs")];
    map.acquire(&"a1".to_string(), &files).unwrap();
    assert!(map.can_write(&"a1".to_string(), &PathBuf::from("src/main.rs")));
}

/// P2b.AC2.5: can_write returns false for files owned by different agent
#[test]
fn test_can_write_different_owner() {
    let mut map = FileOwnershipMap::new();
    let files = vec![PathBuf::from("src/main.rs")];
    map.acquire(&"a1".to_string(), &files).unwrap();
    assert!(!map.can_write(&"a2".to_string(), &PathBuf::from("src/main.rs")));
}

/// P2b.AC2.6: can_write returns true for unowned files
#[test]
fn test_can_write_unowned() {
    let map = FileOwnershipMap::new();
    assert!(map.can_write(&"a1".to_string(), &PathBuf::from("src/anything.rs")));
}

/// P2b.AC3.2: WorkerState has all variants and Debug works
#[test]
fn test_worker_state_variants() {
    let states = vec![
        WorkerState::Spawning,
        WorkerState::Initializing,
        WorkerState::Working,
        WorkerState::Reporting,
        WorkerState::Completed(AgentOutcome::Completed {
            summary: "done".to_string(),
            tokens_used: 0,
        }),
        WorkerState::Failed("error".to_string()),
    ];
    for state in &states {
        let debug = format!("{:?}", state);
        assert!(!debug.is_empty());
    }
}

/// P2b.AC3.1: WorkerHandle fields accessible
#[tokio::test]
async fn test_worker_handle_fields() {
    let cancel_token = tokio_util::sync::CancellationToken::new();
    let handle = tokio::spawn(async { Ok(AgentOutcome::Completed { summary: "done".to_string(), tokens_used: 0 }) });
    let now = chrono::Utc::now();

    let wh = WorkerHandle {
        id: "worker-1".to_string(),
        profile: "coder".to_string(),
        work_package: WorkPackage {
            id: "wp-12345678".to_string(),
            task_ids: vec!["t1".to_string()],
            file_scope: vec![],
            profile: "coder".to_string(),
            priority: Priority::Medium,
            estimated_complexity: Complexity::Small,
        },
        state: WorkerState::Working,
        join_handle: handle,
        cancel_token,
        spawned_at: now,
        last_check_in: now,
    };

    assert_eq!(wh.id, "worker-1");
    assert_eq!(wh.profile, "coder");
    let debug = format!("{:?}", wh);
    assert!(debug.contains("worker-1"));
}

/// P2b.AC4.1: Tasks sharing files are grouped together
#[test]
fn test_group_shared_files() {
    let tasks = vec![
        TaskForGrouping {
            task_id: "t1".to_string(),
            file_scope: vec![PathBuf::from("src/main.rs")],
            profile: "coder".to_string(),
            priority: Priority::Medium,
            depends_on: vec![],
        },
        TaskForGrouping {
            task_id: "t2".to_string(),
            file_scope: vec![PathBuf::from("src/main.rs"), PathBuf::from("src/lib.rs")],
            profile: "coder".to_string(),
            priority: Priority::Medium,
            depends_on: vec![],
        },
    ];
    let packages = group_tasks_into_packages(tasks);
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].task_ids.len(), 2);
}

/// P2b.AC4.2: Tasks with dependencies are grouped together
#[test]
fn test_group_dependent_tasks() {
    let tasks = vec![
        TaskForGrouping {
            task_id: "t1".to_string(),
            file_scope: vec![PathBuf::from("src/a.rs")],
            profile: "coder".to_string(),
            priority: Priority::Medium,
            depends_on: vec![],
        },
        TaskForGrouping {
            task_id: "t2".to_string(),
            file_scope: vec![PathBuf::from("src/b.rs")],
            profile: "coder".to_string(),
            priority: Priority::High,
            depends_on: vec!["t1".to_string()],
        },
    ];
    let packages = group_tasks_into_packages(tasks);
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].task_ids.len(), 2);
    // Highest priority should be used
    assert_eq!(packages[0].priority, Priority::High);
}

/// P2b.AC4.3: Independent tasks produce separate packages
#[test]
fn test_group_independent_tasks() {
    let tasks = vec![
        TaskForGrouping {
            task_id: "t1".to_string(),
            file_scope: vec![PathBuf::from("src/a.rs")],
            profile: "coder".to_string(),
            priority: Priority::Medium,
            depends_on: vec![],
        },
        TaskForGrouping {
            task_id: "t2".to_string(),
            file_scope: vec![PathBuf::from("src/b.rs")],
            profile: "tester".to_string(),
            priority: Priority::Low,
            depends_on: vec![],
        },
    ];
    let packages = group_tasks_into_packages(tasks);
    assert_eq!(packages.len(), 2);
}

/// Work package ID format: wp-{8 hex chars}
#[test]
fn test_work_package_id_format() {
    let id = generate_work_package_id();
    assert!(id.starts_with("wp-"), "ID should start with 'wp-': {}", id);
    let hex_part = &id[3..];
    assert_eq!(hex_part.len(), 8, "Hex part should be 8 chars: {}", hex_part);
    assert!(
        hex_part.chars().all(|c| c.is_ascii_hexdigit()),
        "Should be hex: {}",
        hex_part
    );
}

/// Complexity estimation based on file count
#[test]
fn test_complexity_estimation() {
    // 1 file -> Small
    let tasks = vec![TaskForGrouping {
        task_id: "t1".to_string(),
        file_scope: vec![PathBuf::from("a.rs")],
        profile: "coder".to_string(),
        priority: Priority::Medium,
        depends_on: vec![],
    }];
    let packages = group_tasks_into_packages(tasks);
    assert_eq!(packages[0].estimated_complexity, Complexity::Small);

    // 4 files -> Medium
    let tasks = vec![TaskForGrouping {
        task_id: "t1".to_string(),
        file_scope: vec![
            PathBuf::from("a.rs"),
            PathBuf::from("b.rs"),
            PathBuf::from("c.rs"),
            PathBuf::from("d.rs"),
        ],
        profile: "coder".to_string(),
        priority: Priority::Medium,
        depends_on: vec![],
    }];
    let packages = group_tasks_into_packages(tasks);
    assert_eq!(packages[0].estimated_complexity, Complexity::Medium);

    // 8 files -> Large
    let tasks = vec![TaskForGrouping {
        task_id: "t1".to_string(),
        file_scope: (0..8)
            .map(|i| PathBuf::from(format!("{}.rs", i)))
            .collect(),
        profile: "coder".to_string(),
        priority: Priority::Medium,
        depends_on: vec![],
    }];
    let packages = group_tasks_into_packages(tasks);
    assert_eq!(packages[0].estimated_complexity, Complexity::Large);
}
