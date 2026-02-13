use rustagent::agent::builtin_profiles;
use rustagent::security::scope::{FileOperation, ScopeCheck};

#[test]
fn test_planner_profile_readonly_scope() {
    let profile = builtin_profiles::planner();

    // Planner has read_only: true
    assert!(profile.security.read_only);
    assert!(!profile.security.can_create_files);

    // Should deny writes
    let result = profile.security.check_path("src/main.rs", FileOperation::Write);
    assert_eq!(result, ScopeCheck::Denied("read-only scope".to_string()));

    // Should allow reads
    let result = profile.security.check_path("src/main.rs", FileOperation::Read);
    assert_eq!(result, ScopeCheck::Allowed);

    // Should deny file creation (read-only takes precedence)
    let result = profile.security.check_path("src/newfile.rs", FileOperation::Create);
    assert!(matches!(result, ScopeCheck::Denied(_)));
}

#[test]
fn test_coder_profile_writable_scope() {
    let profile = builtin_profiles::coder();

    // Coder should allow writes
    assert!(!profile.security.read_only);
    assert!(profile.security.can_create_files);

    // Should allow writes
    let result = profile.security.check_path("src/main.rs", FileOperation::Write);
    assert_eq!(result, ScopeCheck::Allowed);

    // Should allow reads
    let result = profile.security.check_path("src/main.rs", FileOperation::Read);
    assert_eq!(result, ScopeCheck::Allowed);

    // Should allow file creation
    let result = profile.security.check_path("src/newfile.rs", FileOperation::Create);
    assert_eq!(result, ScopeCheck::Allowed);
}

#[test]
fn test_coder_profile_commands() {
    let profile = builtin_profiles::coder();

    // Coder allows cargo commands
    let result = profile.security.check_command("cargo test");
    assert_eq!(result, ScopeCheck::Allowed);

    // Coder allows any command (wildcard)
    let result = profile.security.check_command("any command");
    assert_eq!(result, ScopeCheck::Allowed);
}

#[test]
fn test_reviewer_profile_readonly_scope() {
    let profile = builtin_profiles::reviewer();

    // Reviewer has read_only: true
    assert!(profile.security.read_only);
    assert!(!profile.security.can_create_files);

    // Should deny writes
    let result = profile.security.check_path("src/main.rs", FileOperation::Write);
    assert_eq!(result, ScopeCheck::Denied("read-only scope".to_string()));

    // Should allow reads
    let result = profile.security.check_path("src/main.rs", FileOperation::Read);
    assert_eq!(result, ScopeCheck::Allowed);
}

#[test]
fn test_researcher_profile_readonly_scope() {
    let profile = builtin_profiles::researcher();

    // Researcher has read_only: true
    assert!(profile.security.read_only);
    assert!(!profile.security.can_create_files);

    // Should deny writes
    let result = profile.security.check_path("src/main.rs", FileOperation::Write);
    assert_eq!(result, ScopeCheck::Denied("read-only scope".to_string()));

    // Should allow reads
    let result = profile.security.check_path("src/main.rs", FileOperation::Read);
    assert_eq!(result, ScopeCheck::Allowed);
}

#[test]
fn test_tester_profile_writable_scope() {
    let profile = builtin_profiles::tester();

    // Tester should allow writes (for writing tests)
    assert!(!profile.security.read_only);
    assert!(profile.security.can_create_files);

    // Should allow writes
    let result = profile.security.check_path("tests/test.rs", FileOperation::Write);
    assert_eq!(result, ScopeCheck::Allowed);

    // Should allow file creation
    let result = profile.security.check_path("tests/newtest.rs", FileOperation::Create);
    assert_eq!(result, ScopeCheck::Allowed);
}

#[test]
fn test_all_profiles_have_wildcard_paths() {
    // All built-in profiles allow wildcard paths for maximum flexibility
    let profiles = vec![
        builtin_profiles::planner(),
        builtin_profiles::coder(),
        builtin_profiles::reviewer(),
        builtin_profiles::tester(),
        builtin_profiles::researcher(),
    ];

    for profile in profiles {
        // All should have wildcard in allowed_paths
        assert!(
            profile.security.allowed_paths.contains(&"*".to_string()),
            "Profile {} missing wildcard in allowed_paths",
            profile.name
        );

        // All should have wildcard in allowed_commands
        assert!(
            profile.security.allowed_commands.contains(&"*".to_string()),
            "Profile {} missing wildcard in allowed_commands",
            profile.name
        );

        // All should deny network access by default
        assert_eq!(
            profile.security.check_network(),
            ScopeCheck::Denied("network access not allowed".to_string()),
            "Profile {} should deny network access",
            profile.name
        );
    }
}
