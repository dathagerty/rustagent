use rustagent::config::{SecurityConfig, ShellPolicy};
use rustagent::security::{SecurityValidator, ValidationResult};
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_allowlist_policy() {
    let config = SecurityConfig {
        shell_policy: ShellPolicy::Allowlist,
        allowed_commands: vec!["git".to_string(), "cargo".to_string()],
        blocked_patterns: vec![],
        max_file_size_mb: 10,
        allowed_paths: vec![".".to_string()],
    };

    let validator = SecurityValidator::new(config).unwrap();

    // Allowed command
    match validator.validate_shell_command("git status") {
        ValidationResult::Allowed => {}
        _ => panic!("Expected allowed"),
    }

    // Not in allowlist
    match validator.validate_shell_command("rm file") {
        ValidationResult::RequiresPermission(_) => {}
        _ => panic!("Expected permission required"),
    }
}

#[test]
fn test_blocklist_policy() {
    let config = SecurityConfig {
        shell_policy: ShellPolicy::Blocklist,
        allowed_commands: vec![],
        blocked_patterns: vec!["rm -rf".to_string(), "eval.*".to_string()],
        max_file_size_mb: 10,
        allowed_paths: vec![".".to_string()],
    };

    let validator = SecurityValidator::new(config).unwrap();

    // Safe command
    match validator.validate_shell_command("ls -la") {
        ValidationResult::Allowed => {}
        _ => panic!("Expected allowed"),
    }

    // Blocked pattern
    match validator.validate_shell_command("rm -rf /") {
        ValidationResult::Denied(_) => {}
        _ => panic!("Expected denied"),
    }
}

#[test]
fn test_path_validation() {
    let config = SecurityConfig {
        shell_policy: ShellPolicy::Allowlist,
        allowed_commands: vec![],
        blocked_patterns: vec![],
        max_file_size_mb: 10,
        allowed_paths: vec![".".to_string()],
    };

    let validator = SecurityValidator::new(config).unwrap();

    // Path in current directory
    match validator.validate_file_path(&PathBuf::from("./test.txt")) {
        ValidationResult::Allowed => {}
        _ => panic!("Expected allowed"),
    }
}

#[test]
fn test_nested_nonexistent_dirs() {
    let temp_dir = TempDir::new().unwrap();
    let config = SecurityConfig {
        shell_policy: ShellPolicy::Allowlist,
        allowed_commands: vec![],
        blocked_patterns: vec![],
        max_file_size_mb: 10,
        allowed_paths: vec![temp_dir.path().to_string_lossy().to_string()],
    };

    let validator = SecurityValidator::new(config).unwrap();

    let nested_path = temp_dir.path().join("new").join("sub").join("file.txt");
    match validator.validate_file_path(&nested_path) {
        ValidationResult::Allowed => {}
        result => panic!("Expected allowed, got {:?}", result),
    }
}

#[test]
fn test_traversal_in_suffix_denied() {
    let temp_dir = TempDir::new().unwrap();
    let config = SecurityConfig {
        shell_policy: ShellPolicy::Allowlist,
        allowed_commands: vec![],
        blocked_patterns: vec![],
        max_file_size_mb: 10,
        allowed_paths: vec![temp_dir.path().to_string_lossy().to_string()],
    };

    let validator = SecurityValidator::new(config).unwrap();

    let traversal_path = temp_dir
        .path()
        .join("new")
        .join("..")
        .join("..")
        .join("etc")
        .join("passwd");
    match validator.validate_file_path(&traversal_path) {
        ValidationResult::Denied(msg) => {
            assert!(
                msg.contains("traversal"),
                "Expected traversal error, got: {}",
                msg
            );
        }
        result => panic!("Expected denied, got {:?}", result),
    }
}

#[test]
fn test_dot_component_normalized() {
    let temp_dir = TempDir::new().unwrap();
    let config = SecurityConfig {
        shell_policy: ShellPolicy::Allowlist,
        allowed_commands: vec![],
        blocked_patterns: vec![],
        max_file_size_mb: 10,
        allowed_paths: vec![temp_dir.path().to_string_lossy().to_string()],
    };

    let validator = SecurityValidator::new(config).unwrap();

    let dot_path = temp_dir
        .path()
        .join("new")
        .join(".")
        .join("sub")
        .join("file.txt");
    match validator.validate_file_path(&dot_path) {
        ValidationResult::Allowed => {}
        result => panic!("Expected allowed (dot normalized), got {:?}", result),
    }
}

#[test]
fn test_existing_path_no_regression() {
    let temp_dir = TempDir::new().unwrap();
    let existing_file = temp_dir.path().join("existing.txt");
    std::fs::write(&existing_file, "test").unwrap();

    let config = SecurityConfig {
        shell_policy: ShellPolicy::Allowlist,
        allowed_commands: vec![],
        blocked_patterns: vec![],
        max_file_size_mb: 10,
        allowed_paths: vec![temp_dir.path().to_string_lossy().to_string()],
    };

    let validator = SecurityValidator::new(config).unwrap();

    match validator.validate_file_path(&existing_file) {
        ValidationResult::Allowed => {}
        result => panic!("Expected allowed for existing file, got {:?}", result),
    }
}

#[test]
fn test_path_outside_allowed_requires_permission() {
    let temp_dir = TempDir::new().unwrap();
    let config = SecurityConfig {
        shell_policy: ShellPolicy::Allowlist,
        allowed_commands: vec![],
        blocked_patterns: vec![],
        max_file_size_mb: 10,
        allowed_paths: vec![temp_dir.path().to_string_lossy().to_string()],
    };

    let validator = SecurityValidator::new(config).unwrap();

    let outside_path = PathBuf::from("/tmp/outside/file.txt");
    match validator.validate_file_path(&outside_path) {
        ValidationResult::RequiresPermission(_) => {}
        result => panic!("Expected requires permission, got {:?}", result),
    }
}

#[test]
fn test_traversal_via_existing_path() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::create_dir(temp_dir.path().join("subdir")).unwrap();

    let config = SecurityConfig {
        shell_policy: ShellPolicy::Allowlist,
        allowed_commands: vec![],
        blocked_patterns: vec![],
        max_file_size_mb: 10,
        allowed_paths: vec![temp_dir.path().join("subdir").to_string_lossy().to_string()],
    };

    let validator = SecurityValidator::new(config).unwrap();

    let traversal_path = temp_dir
        .path()
        .join("subdir")
        .join("..")
        .join("outside.txt");
    match validator.validate_file_path(&traversal_path) {
        ValidationResult::RequiresPermission(_) => {}
        result => panic!(
            "Expected requires permission for path outside allowed dirs, got {:?}",
            result
        ),
    }
}
