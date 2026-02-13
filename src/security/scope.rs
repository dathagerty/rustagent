use glob::{MatchOptions, Pattern};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Represents the type of file operation being checked
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOperation {
    Read,
    Write,
    Create,
}

/// Result of a security scope check
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeCheck {
    Allowed,
    Denied(String),
}

/// Match options that allow `**` to match across directory separators.
fn glob_match_options() -> MatchOptions {
    MatchOptions {
        case_sensitive: true,
        require_literal_separator: false, // allows `**` to match `/`
        require_literal_leading_dot: false,
    }
}

/// Security scope for an agent, controlling what it can access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityScope {
    /// Paths the agent is allowed to access
    pub allowed_paths: Vec<String>,

    /// Paths the agent is explicitly denied access to
    pub denied_paths: Vec<String>,

    /// Shell commands the agent is allowed to run
    pub allowed_commands: Vec<String>,

    /// Whether the agent has read-only access
    pub read_only: bool,

    /// Whether the agent can create new files
    pub can_create_files: bool,

    /// Whether the agent can access network resources
    pub network_access: bool,
}

impl Default for SecurityScope {
    fn default() -> Self {
        Self {
            allowed_paths: vec!["*".to_string()],
            denied_paths: vec![],
            allowed_commands: vec!["*".to_string()],
            read_only: false,
            can_create_files: true,
            network_access: false,
        }
    }
}

impl SecurityScope {
    /// Check whether a file operation is permitted for the given path.
    pub fn check_path(&self, path: &str, operation: FileOperation) -> ScopeCheck {
        // Check read_only constraint
        if self.read_only && matches!(operation, FileOperation::Write | FileOperation::Create) {
            return ScopeCheck::Denied("read-only scope".to_string());
        }

        // Check can_create_files constraint
        if !self.can_create_files && operation == FileOperation::Create {
            return ScopeCheck::Denied("file creation not allowed".to_string());
        }

        let opts = glob_match_options();
        let path_ref = Path::new(path);

        // Check denied patterns first (deny takes precedence)
        for pattern_str in &self.denied_paths {
            if let Ok(pattern) = Pattern::new(pattern_str)
                && pattern.matches_path_with(path_ref, opts)
            {
                return ScopeCheck::Denied(format!("path matches denied pattern: {}", pattern_str));
            }
        }

        // Check allowed patterns
        for pattern_str in &self.allowed_paths {
            if let Ok(pattern) = Pattern::new(pattern_str)
                && pattern.matches_path_with(path_ref, opts)
            {
                return ScopeCheck::Allowed;
            }
        }

        ScopeCheck::Denied("path not in allowed patterns".to_string())
    }

    /// Check whether a shell command is permitted.
    pub fn check_command(&self, command: &str) -> ScopeCheck {
        for pattern_str in &self.allowed_commands {
            if let Ok(pattern) = Pattern::new(pattern_str)
                && pattern.matches(command)
            {
                return ScopeCheck::Allowed;
            }
        }

        ScopeCheck::Denied(format!("command not in allowed patterns: {}", command))
    }

    /// Check whether network access is permitted.
    pub fn check_network(&self) -> ScopeCheck {
        if self.network_access {
            ScopeCheck::Allowed
        } else {
            ScopeCheck::Denied("network access not allowed".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============ check_path tests ============

    #[test]
    fn test_check_path_allowed_simple() {
        let scope = SecurityScope {
            allowed_paths: vec!["src/**".to_string()],
            denied_paths: vec![],
            allowed_commands: vec![],
            read_only: false,
            can_create_files: true,
            network_access: false,
        };

        let result = scope.check_path("src/main.rs", FileOperation::Read);
        assert_eq!(result, ScopeCheck::Allowed);
    }

    #[test]
    fn test_check_path_denied_pattern_precedence() {
        let scope = SecurityScope {
            allowed_paths: vec!["src/**".to_string()],
            denied_paths: vec!["src/private/**".to_string()],
            allowed_commands: vec![],
            read_only: false,
            can_create_files: true,
            network_access: false,
        };

        // Path matches allowed pattern but also denied pattern - deny should win
        let result = scope.check_path("src/private/secret.rs", FileOperation::Read);
        assert!(matches!(result, ScopeCheck::Denied(_)));
    }

    #[test]
    fn test_check_path_read_only_blocks_write() {
        let scope = SecurityScope {
            allowed_paths: vec!["*".to_string()],
            denied_paths: vec![],
            allowed_commands: vec![],
            read_only: true,
            can_create_files: false,
            network_access: false,
        };

        // Write operation should be blocked by read_only
        let result = scope.check_path("src/main.rs", FileOperation::Write);
        assert_eq!(result, ScopeCheck::Denied("read-only scope".to_string()));
    }

    #[test]
    fn test_check_path_read_only_allows_read() {
        let scope = SecurityScope {
            allowed_paths: vec!["*".to_string()],
            denied_paths: vec![],
            allowed_commands: vec![],
            read_only: true,
            can_create_files: false,
            network_access: false,
        };

        // Read operation should be allowed even when read_only
        let result = scope.check_path("src/main.rs", FileOperation::Read);
        assert_eq!(result, ScopeCheck::Allowed);
    }

    #[test]
    fn test_check_path_create_blocked_when_disabled() {
        let scope = SecurityScope {
            allowed_paths: vec!["*".to_string()],
            denied_paths: vec![],
            allowed_commands: vec![],
            read_only: false,
            can_create_files: false,
            network_access: false,
        };

        // Create operation should be blocked when can_create_files is false
        let result = scope.check_path("src/newfile.rs", FileOperation::Create);
        assert_eq!(
            result,
            ScopeCheck::Denied("file creation not allowed".to_string())
        );
    }

    #[test]
    fn test_check_path_write_allowed_when_not_readonly() {
        let scope = SecurityScope {
            allowed_paths: vec!["*".to_string()],
            denied_paths: vec![],
            allowed_commands: vec![],
            read_only: false,
            can_create_files: true,
            network_access: false,
        };

        // Write operation should be allowed
        let result = scope.check_path("src/main.rs", FileOperation::Write);
        assert_eq!(result, ScopeCheck::Allowed);
    }

    #[test]
    fn test_check_path_wildcard_matches_everything() {
        let scope = SecurityScope {
            allowed_paths: vec!["*".to_string()],
            denied_paths: vec![],
            allowed_commands: vec![],
            read_only: false,
            can_create_files: true,
            network_access: false,
        };

        let result = scope.check_path("anything/anywhere.txt", FileOperation::Read);
        assert_eq!(result, ScopeCheck::Allowed);
    }

    #[test]
    fn test_check_path_recursive_pattern() {
        let scope = SecurityScope {
            allowed_paths: vec!["src/**".to_string()],
            denied_paths: vec![],
            allowed_commands: vec![],
            read_only: false,
            can_create_files: true,
            network_access: false,
        };

        // Test that src/** matches nested paths
        let result = scope.check_path("src/auth/handler.rs", FileOperation::Read);
        assert_eq!(result, ScopeCheck::Allowed);
    }

    #[test]
    fn test_check_path_not_in_allowed() {
        let scope = SecurityScope {
            allowed_paths: vec!["src/**".to_string()],
            denied_paths: vec![],
            allowed_commands: vec![],
            read_only: false,
            can_create_files: true,
            network_access: false,
        };

        let result = scope.check_path("tests/something.rs", FileOperation::Read);
        assert!(
            matches!(result, ScopeCheck::Denied(ref msg) if msg.contains("path not in allowed patterns"))
        );
    }

    // ============ check_command tests ============

    #[test]
    fn test_check_command_allowed() {
        let scope = SecurityScope {
            allowed_paths: vec![],
            denied_paths: vec![],
            allowed_commands: vec!["cargo *".to_string()],
            read_only: false,
            can_create_files: true,
            network_access: false,
        };

        let result = scope.check_command("cargo test");
        assert_eq!(result, ScopeCheck::Allowed);
    }

    #[test]
    fn test_check_command_denied() {
        let scope = SecurityScope {
            allowed_paths: vec![],
            denied_paths: vec![],
            allowed_commands: vec!["cargo *".to_string()],
            read_only: false,
            can_create_files: true,
            network_access: false,
        };

        let result = scope.check_command("rm -rf /");
        assert!(
            matches!(result, ScopeCheck::Denied(ref msg) if msg.contains("command not in allowed patterns"))
        );
    }

    #[test]
    fn test_check_command_wildcard() {
        let scope = SecurityScope {
            allowed_paths: vec![],
            denied_paths: vec![],
            allowed_commands: vec!["*".to_string()],
            read_only: false,
            can_create_files: true,
            network_access: false,
        };

        // Wildcard should match anything
        let result = scope.check_command("any command here");
        assert_eq!(result, ScopeCheck::Allowed);
    }

    #[test]
    fn test_check_command_exact_match() {
        let scope = SecurityScope {
            allowed_paths: vec![],
            denied_paths: vec![],
            allowed_commands: vec!["cargo test".to_string()],
            read_only: false,
            can_create_files: true,
            network_access: false,
        };

        let result = scope.check_command("cargo test");
        assert_eq!(result, ScopeCheck::Allowed);
    }

    // ============ check_network tests ============

    #[test]
    fn test_check_network_allowed() {
        let scope = SecurityScope {
            allowed_paths: vec![],
            denied_paths: vec![],
            allowed_commands: vec![],
            read_only: false,
            can_create_files: true,
            network_access: true,
        };

        let result = scope.check_network();
        assert_eq!(result, ScopeCheck::Allowed);
    }

    #[test]
    fn test_check_network_denied() {
        let scope = SecurityScope {
            allowed_paths: vec![],
            denied_paths: vec![],
            allowed_commands: vec![],
            read_only: false,
            can_create_files: true,
            network_access: false,
        };

        let result = scope.check_network();
        assert_eq!(
            result,
            ScopeCheck::Denied("network access not allowed".to_string())
        );
    }
}
