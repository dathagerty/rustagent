use crate::config::{SecurityConfig, ShellPolicy};
use anyhow::{anyhow, Result};
use regex::Regex;
use std::path::{Path, PathBuf};

pub struct SecurityValidator {
    config: SecurityConfig,
    blocked_regexes: Vec<Regex>,
    allowed_paths_canonical: Vec<PathBuf>,
}

#[derive(Debug)]
pub enum ValidationResult {
    Allowed,
    Denied(String),
    RequiresPermission(String),
}

impl SecurityValidator {
    pub fn new(config: SecurityConfig) -> Result<Self> {
        // Compile blocked patterns into regexes
        let blocked_regexes = config
            .blocked_patterns
            .iter()
            .map(|p| Regex::new(p))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow!("Invalid regex pattern: {}", e))?;

        // Canonicalize allowed paths
        let allowed_paths_canonical = config
            .allowed_paths
            .iter()
            .map(|p| {
                let expanded = shellexpand::tilde(p);
                PathBuf::from(expanded.as_ref())
                    .canonicalize()
                    .unwrap_or_else(|_| PathBuf::from(expanded.as_ref()))
            })
            .collect();

        Ok(Self {
            config,
            blocked_regexes,
            allowed_paths_canonical,
        })
    }

    pub fn validate_shell_command(&self, command: &str) -> ValidationResult {
        match self.config.shell_policy {
            ShellPolicy::Unrestricted => ValidationResult::Allowed,

            ShellPolicy::Allowlist => {
                // Extract base command (first word)
                let base_cmd = command.split_whitespace().next().unwrap_or("");

                if self.config.allowed_commands.contains(&base_cmd.to_string()) {
                    ValidationResult::Allowed
                } else {
                    ValidationResult::RequiresPermission(format!(
                        "Command '{}' not in allowlist",
                        base_cmd
                    ))
                }
            }

            ShellPolicy::Blocklist => {
                for pattern in &self.blocked_regexes {
                    if pattern.is_match(command) {
                        return ValidationResult::Denied(format!(
                            "Command matches blocked pattern: {}",
                            pattern
                        ));
                    }
                }
                ValidationResult::Allowed
            }
        }
    }

    pub fn validate_file_path(&self, path: &Path) -> ValidationResult {
        // Canonicalize the requested path
        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                // Path doesn't exist yet, try to canonicalize parent
                if let Some(parent) = path.parent() {
                    match parent.canonicalize() {
                        Ok(p) => p.join(path.file_name().unwrap()),
                        Err(_) => {
                            return ValidationResult::Denied("Cannot resolve path".to_string())
                        }
                    }
                } else {
                    return ValidationResult::Denied("Invalid path".to_string());
                }
            }
        };

        // Check if path is within allowed paths
        for allowed in &self.allowed_paths_canonical {
            if canonical.starts_with(allowed) {
                return ValidationResult::Allowed;
            }
        }

        ValidationResult::RequiresPermission(format!(
            "Path '{}' is outside allowed directories",
            path.display()
        ))
    }

    pub fn check_file_size(&self, path: &Path) -> ValidationResult {
        match std::fs::metadata(path) {
            Ok(metadata) => {
                let size_mb = metadata.len() / (1024 * 1024);
                if size_mb <= self.config.max_file_size_mb {
                    ValidationResult::Allowed
                } else {
                    ValidationResult::Denied(format!(
                        "File size {}MB exceeds limit of {}MB",
                        size_mb, self.config.max_file_size_mb
                    ))
                }
            }
            Err(e) => ValidationResult::Denied(format!("Cannot check file size: {}", e)),
        }
    }
}
