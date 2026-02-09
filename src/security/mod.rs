use crate::config::{SecurityConfig, ShellPolicy};
use anyhow::{Result, anyhow};
use regex::Regex;
use std::path::{Component, Path, PathBuf};

pub mod permission;
pub mod scope;

pub use scope::SecurityScope;

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
        let path_str = path.to_string_lossy();
        let expanded = shellexpand::tilde(&path_str);
        let path = PathBuf::from(expanded.as_ref());

        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => match resolve_nonexistent_path(&path) {
                Ok(p) => p,
                Err(reason) => return ValidationResult::Denied(reason),
            },
        };

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

fn resolve_nonexistent_path(path: &Path) -> Result<PathBuf, String> {
    let components: Vec<Component> = path.components().collect();

    for i in (0..=components.len()).rev() {
        let ancestor: PathBuf = components[..i].iter().collect();

        if ancestor.as_os_str().is_empty() {
            if let Ok(canonical) = std::env::current_dir() {
                let suffix = &components[i..];
                return validate_and_build_path(canonical, suffix);
            }
            continue;
        }

        if let Ok(canonical) = ancestor.canonicalize() {
            let suffix = &components[i..];
            return validate_and_build_path(canonical, suffix);
        }
    }

    Err("cannot resolve path".to_string())
}

fn validate_and_build_path(base: PathBuf, suffix: &[Component]) -> Result<PathBuf, String> {
    let mut resolved = base;

    for component in suffix {
        match component {
            Component::ParentDir => {
                return Err("path contains invalid traversal (..)".to_string());
            }
            Component::CurDir => {
                continue;
            }
            Component::Normal(name) => {
                resolved = resolved.join(name);
            }
            _ => {
                return Err("invalid path component in suffix".to_string());
            }
        }
    }

    Ok(resolved)
}
