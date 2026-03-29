use rustagent::agent::worktree::WorktreeManager;
use std::process::Command;
use tempfile::TempDir;

mod common;

/// Initialize a git repo in a temp directory with an initial commit.
fn init_test_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    Command::new("git")
        .args(["init"])
        .current_dir(&path)
        .output()
        .unwrap();

    // Configure git user for commits
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "--allow-empty", "-m", "initial"])
        .current_dir(&path)
        .output()
        .unwrap();

    (dir, path)
}

/// Get list of git branches as strings.
fn get_branches(path: &std::path::Path) -> Vec<String> {
    let output = Command::new("git")
        .args(["branch", "--list"])
        .current_dir(path)
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| l.trim().trim_start_matches("* ").to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

// ===== Goal Branch Tests =====

/// P2f.AC1.1: create_goal_branch creates branch from HEAD
#[test]
fn test_create_goal_branch() {
    let (_dir, path) = init_test_repo();
    let wm = WorktreeManager::new(path.clone());

    let branch = wm.create_goal_branch("ra-test").unwrap();
    assert_eq!(branch, "rustagent/ra-test");

    let branches = get_branches(&path);
    assert!(branches.contains(&"rustagent/ra-test".to_string()));
}

/// P2f.AC1.2: create_goal_branch twice — no error on second call
#[test]
fn test_create_goal_branch_idempotent() {
    let (_dir, path) = init_test_repo();
    let wm = WorktreeManager::new(path.clone());

    let b1 = wm.create_goal_branch("ra-test").unwrap();
    let b2 = wm.create_goal_branch("ra-test").unwrap();
    assert_eq!(b1, b2);
}

/// P2f: goal_branch_name returns expected format
#[test]
fn test_goal_branch_name() {
    assert_eq!(
        WorktreeManager::goal_branch_name("ra-abcd"),
        "rustagent/ra-abcd"
    );
}

// ===== Worktree Tests =====

/// P2f.AC2.1: create_worktree creates directory at conventional path
#[test]
fn test_create_worktree_directory_exists() {
    let (_dir, path) = init_test_repo();
    let wm = WorktreeManager::new(path.clone());

    wm.create_goal_branch("ra-test").unwrap();
    let wt_path = wm.create_worktree("ra-test", "wp-001").unwrap();

    assert!(wt_path.exists());
    assert!(wt_path.is_dir());
}

/// P2f.AC2.2: create_worktree creates a branch for the work package
#[test]
fn test_create_worktree_creates_branch() {
    let (_dir, path) = init_test_repo();
    let wm = WorktreeManager::new(path.clone());

    wm.create_goal_branch("ra-test").unwrap();
    wm.create_worktree("ra-test", "wp-001").unwrap();

    let branches = get_branches(&path);
    assert!(
        branches.iter().any(|b| b.contains("wp-wp-001")),
        "Expected a wp-wp-001 branch, found: {:?}",
        branches
    );
}

/// P2f.AC2: Worktree is a valid git checkout
#[test]
fn test_worktree_is_valid_checkout() {
    let (_dir, path) = init_test_repo();
    let wm = WorktreeManager::new(path.clone());

    wm.create_goal_branch("ra-test").unwrap();
    let wt_path = wm.create_worktree("ra-test", "wp-001").unwrap();

    // Should be able to run git status in the worktree
    let output = Command::new("git")
        .args(["status"])
        .current_dir(&wt_path)
        .output()
        .unwrap();
    assert!(output.status.success());
}

// ===== Merge Tests =====

/// P2f.AC3.1: merge_work_package merges wp branch into goal branch
#[test]
fn test_merge_work_package() {
    let (_dir, path) = init_test_repo();
    let wm = WorktreeManager::new(path.clone());

    wm.create_goal_branch("ra-test").unwrap();
    let wt_path = wm.create_worktree("ra-test", "wp-001").unwrap();

    // Create a file in the worktree and commit
    std::fs::write(wt_path.join("new_file.txt"), "hello from worker").unwrap();
    Command::new("git")
        .args(["add", "new_file.txt"])
        .current_dir(&wt_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "worker: add new_file.txt"])
        .current_dir(&wt_path)
        .output()
        .unwrap();

    // Merge the work package into the goal branch
    wm.merge_work_package("ra-test", "wp-001").unwrap();

    // Verify: check out goal branch in a temp worktree and confirm the file exists
    let verify_path = path.join(".rustagent").join("worktrees").join("verify");
    let output = Command::new("git")
        .args([
            "worktree",
            "add",
            verify_path.to_str().unwrap(),
            "rustagent/ra-test",
        ])
        .current_dir(&path)
        .output()
        .unwrap();
    assert!(output.status.success(), "Failed to create verify worktree");

    assert!(verify_path.join("new_file.txt").exists());
    let content = std::fs::read_to_string(verify_path.join("new_file.txt")).unwrap();
    assert_eq!(content, "hello from worker");

    // Cleanup verify worktree
    let _ = Command::new("git")
        .args([
            "worktree",
            "remove",
            "--force",
            verify_path.to_str().unwrap(),
        ])
        .current_dir(&path)
        .output();
}

/// P2f.AC3.2: cleanup_worktree removes directory and branch
#[test]
fn test_cleanup_worktree() {
    let (_dir, path) = init_test_repo();
    let wm = WorktreeManager::new(path.clone());

    wm.create_goal_branch("ra-test").unwrap();
    let wt_path = wm.create_worktree("ra-test", "wp-002").unwrap();
    assert!(wt_path.exists());

    // Need a commit for the merge to succeed
    std::fs::write(wt_path.join("test.txt"), "test").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(&wt_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "test commit"])
        .current_dir(&wt_path)
        .output()
        .unwrap();

    wm.merge_work_package("ra-test", "wp-002").unwrap();
    wm.cleanup_worktree("ra-test", "wp-002").unwrap();

    // Worktree directory should be removed
    assert!(!wt_path.exists());

    // Branch should be deleted
    let branches = get_branches(&path);
    assert!(!branches.iter().any(|b| b.contains("wp-wp-002")));
}

// ===== .gitignore Tests =====

/// P2f: create_goal_branch creates .gitignore entry
#[test]
fn test_gitignore_created() {
    let (_dir, path) = init_test_repo();
    let wm = WorktreeManager::new(path.clone());

    wm.create_goal_branch("ra-test").unwrap();

    let gitignore = std::fs::read_to_string(path.join(".gitignore")).unwrap();
    assert!(gitignore.contains(".rustagent/worktrees/"));
}

/// P2f: ensure_gitignore is idempotent
#[test]
fn test_gitignore_idempotent() {
    let (_dir, path) = init_test_repo();
    let wm = WorktreeManager::new(path.clone());

    // Create goal branch twice (which calls ensure_gitignore twice)
    wm.create_goal_branch("ra-test1").unwrap();
    wm.create_goal_branch("ra-test2").unwrap();

    let gitignore = std::fs::read_to_string(path.join(".gitignore")).unwrap();
    let count = gitignore.matches(".rustagent/worktrees/").count();
    assert_eq!(count, 1);
}

/// P2f: ensure_gitignore appends to existing .gitignore
#[test]
fn test_gitignore_appends_to_existing() {
    let (_dir, path) = init_test_repo();

    // Create an existing .gitignore
    std::fs::write(path.join(".gitignore"), "target/\n*.log\n").unwrap();

    let wm = WorktreeManager::new(path.clone());
    wm.create_goal_branch("ra-test").unwrap();

    let gitignore = std::fs::read_to_string(path.join(".gitignore")).unwrap();
    assert!(gitignore.contains("target/"));
    assert!(gitignore.contains("*.log"));
    assert!(gitignore.contains(".rustagent/worktrees/"));
}

// ===== Single-Agent Fallback =====

/// P2f.AC4.1: When max_concurrent_workers=1, orchestrator has no worktree manager
/// (verified indirectly — completing summary won't mention a branch)
#[tokio::test]
async fn test_single_agent_no_worktree_branch_in_summary() {
    let (_, graph_store) = common::setup_test_env().await.unwrap();
    let graph_store: Arc<dyn rustagent::graph::store::GraphStore> = Arc::new(graph_store);

    use rustagent::agent::orchestrator::{Orchestrator, OrchestratorConfig};
    use rustagent::config::{SecurityConfig, ShellPolicy};
    use rustagent::llm::mock::MockLlmClient;
    use rustagent::message::{MessageBus, TokioMessageBus};
    use rustagent::security::SecurityValidator;
    use rustagent::security::permission::AutoApproveHandler;
    use std::sync::Arc;

    let message_bus: Arc<dyn MessageBus> = Arc::new(TokioMessageBus::default());
    let mock_client = Arc::new(MockLlmClient::new());
    let security_config = SecurityConfig {
        shell_policy: ShellPolicy::Unrestricted,
        allowed_commands: vec![],
        blocked_patterns: vec![],
        max_file_size_mb: 100,
        allowed_paths: vec![],
    };
    let validator = Arc::new(SecurityValidator::new(security_config).unwrap());
    let permission_handler = Arc::new(AutoApproveHandler);

    // Single-agent mode: max_concurrent_workers = 1
    let config = OrchestratorConfig {
        max_concurrent_workers: 1,
        ..OrchestratorConfig::default()
    };

    let mut orchestrator = Orchestrator::new(
        config,
        graph_store.clone(),
        message_bus,
        mock_client,
        validator,
        permission_handler,
        std::path::PathBuf::from("/tmp/test"),
        "proj-1".to_string(),
    );

    // Manually set goal to test completing
    orchestrator.set_goal_id(Some("ra-single".to_string()));

    let result = orchestrator.handle_completing().await.unwrap();

    // In single-agent mode, summary should NOT mention a branch
    assert!(!result.summary.contains("Changes are on branch"));
}
