use crate::tools::Tool;
use anyhow::{Context, Result};
use async_trait::async_trait;
use glob::Pattern;
use regex::Regex;
use serde_json::json;
use std::path::PathBuf;
use walkdir::WalkDir;

pub struct CodeSearchTool {
    project_root: PathBuf,
}

impl CodeSearchTool {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }
}

#[async_trait]
impl Tool for CodeSearchTool {
    fn name(&self) -> &str {
        "code_search"
    }

    fn description(&self) -> &str {
        "Searches for a regex pattern in file contents within the project, supporting file glob filtering and directory scoping"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for in file contents"
                },
                "file_glob": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g., '*.rs', '*.ts')"
                },
                "directory": {
                    "type": "string",
                    "description": "Subdirectory to scope the search (relative to project root)"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of matching lines to return (default: 50)"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> Result<String> {
        // Parse parameters
        let pattern_str = params["pattern"]
            .as_str()
            .context("Missing or invalid 'pattern' parameter")?;

        let file_glob = params["file_glob"].as_str();
        let directory = params["directory"].as_str();
        let max_results = params["max_results"]
            .as_u64()
            .unwrap_or(50) as usize;

        // Compile regex pattern
        let regex = Regex::new(pattern_str)
            .context(format!("Invalid regex pattern: {}", pattern_str))?;

        // Compile glob pattern if provided
        let glob_pattern = file_glob
            .map(Pattern::new)
            .transpose()
            .context("Invalid glob pattern")?;

        // Determine search root
        let search_root = if let Some(dir) = directory {
            self.project_root.join(dir)
        } else {
            self.project_root.clone()
        };

        if !search_root.exists() {
            anyhow::bail!(
                "Search directory does not exist: {}",
                search_root.display()
            );
        }

        // Hidden directories to skip
        let skip_dirs = [".git", ".jj", "node_modules", "target"];

        let mut results = Vec::new();
        let mut match_count = 0;
        let mut capped = false;

        // Walk directory tree
        for entry in WalkDir::new(&search_root)
            .into_iter()
            .filter_entry(|e| {
                // Skip hidden directories
                let file_name = e.file_name().to_string_lossy();
                if e.file_type().is_dir() && file_name.starts_with('.') {
                    let name_str = file_name.as_ref();
                    !skip_dirs.contains(&name_str)
                } else {
                    true
                }
            })
        {
            let entry = entry?;
            let path = entry.path();

            // Skip directories
            if path.is_dir() {
                continue;
            }

            // Check glob filter if provided
            if let Some(ref pattern) = glob_pattern {
                let file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                if !pattern.matches(file_name) {
                    continue;
                }
            }

            // Try to read file as UTF-8, skip if it fails (binary files)
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue, // Skip binary files
            };

            // Search for matches in this file
            let relative_path = path
                .strip_prefix(&search_root)
                .unwrap_or(path)
                .display()
                .to_string();

            for (line_num, line) in content.lines().enumerate() {
                if regex.is_match(line) {
                    match_count += 1;
                    if match_count <= max_results {
                        results.push(format!(
                            "{}:{}: {}",
                            relative_path,
                            line_num + 1,
                            line.trim()
                        ));
                    } else {
                        capped = true;
                        break;
                    }
                }
            }

            if capped {
                break;
            }
        }

        // Format output
        if results.is_empty() {
            return Ok("No matches found".to_string());
        }

        let mut output = results.join("\n");
        if capped {
            output.push('\n');
            output.push_str(&format!(
                "Found {} matches (limited to max_results: {})",
                match_count, max_results
            ));
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn code_search_finds_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();

        // Create test file
        let main_rs = project_root.join("main.rs");
        std::fs::write(&main_rs, "fn main() {\n    println!(\"hello\");\n}").unwrap();

        let tool = CodeSearchTool::new(project_root);
        let params = json!({
            "pattern": "fn main"
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("main.rs:1:"));
        assert!(result.contains("fn main"));
    }

    #[tokio::test]
    async fn code_search_with_file_glob() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();

        // Create test files
        std::fs::write(project_root.join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(project_root.join("lib.rs"), "pub fn add(a: i32, b: i32) -> i32 { a + b }").unwrap();
        std::fs::write(project_root.join("README.md"), "# My Project\npub fn should_not_match").unwrap();

        let tool = CodeSearchTool::new(project_root);
        let params = json!({
            "pattern": "pub fn",
            "file_glob": "*.rs"
        });

        let result = tool.execute(params).await.unwrap();
        // Should find in lib.rs
        assert!(result.contains("lib.rs"));
        // Should NOT find in README.md
        assert!(!result.contains("README.md"));
        // Should NOT find main.rs (no "pub fn" there)
        assert!(!result.contains("main.rs"));
    }

    #[tokio::test]
    async fn code_search_respects_max_results() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();

        // Create test file with multiple matches
        std::fs::write(
            project_root.join("test.rs"),
            "fn one()\nfn two()\nfn three()\nfn four()\nfn five()",
        ).unwrap();

        let tool = CodeSearchTool::new(project_root);
        let params = json!({
            "pattern": "fn",
            "max_results": 2
        });

        let result = tool.execute(params).await.unwrap();
        let lines: Vec<&str> = result.lines().collect();
        // Should have 2 match lines + 1 capped message line = 3 total
        assert_eq!(lines.len(), 3);
        assert!(result.contains("Found 3 matches (limited to max_results: 2)"));
    }

    #[tokio::test]
    async fn code_search_skips_binary_files() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();

        // Create a text file
        std::fs::write(project_root.join("text.txt"), "hello world").unwrap();
        // Create a binary file (random bytes)
        std::fs::write(project_root.join("binary.bin"), &[0xFF, 0xFE, 0xFD]).unwrap();

        let tool = CodeSearchTool::new(project_root);
        let params = json!({
            "pattern": "."
        });

        // Should not fail, just skip the binary
        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("text.txt") || result.contains("No matches found"));
    }

    #[tokio::test]
    async fn code_search_with_subdirectory() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();

        // Create directory structure
        std::fs::create_dir_all(project_root.join("src/utils")).unwrap();
        std::fs::write(project_root.join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(project_root.join("src/utils/helper.rs"), "pub fn helper() {}").unwrap();

        let tool = CodeSearchTool::new(project_root);
        let params = json!({
            "pattern": "fn",
            "directory": "src"
        });

        let result = tool.execute(params).await.unwrap();
        assert!(result.contains("utils/helper.rs"));
        assert!(!result.contains("main.rs"));
    }

    #[tokio::test]
    async fn code_search_no_matches() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();

        std::fs::write(project_root.join("test.rs"), "fn main() {}").unwrap();

        let tool = CodeSearchTool::new(project_root);
        let params = json!({
            "pattern": "nonexistent_pattern"
        });

        let result = tool.execute(params).await.unwrap();
        assert_eq!(result, "No matches found");
    }
}
