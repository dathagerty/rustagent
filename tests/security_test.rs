use rustagent::config::{SecurityConfig, ShellPolicy};
use rustagent::security::{SecurityValidator, ValidationResult};
use std::path::PathBuf;

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
        ValidationResult::Allowed => {},
        _ => panic!("Expected allowed"),
    }

    // Not in allowlist
    match validator.validate_shell_command("rm file") {
        ValidationResult::RequiresPermission(_) => {},
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
        ValidationResult::Allowed => {},
        _ => panic!("Expected allowed"),
    }

    // Blocked pattern
    match validator.validate_shell_command("rm -rf /") {
        ValidationResult::Denied(_) => {},
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
        ValidationResult::Allowed => {},
        _ => panic!("Expected allowed"),
    }
}
