use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Resolve AGENTS.md files for a given scope of files
///
/// This walks the directory hierarchy from the project root towards each file in scope,
/// collecting all AGENTS.md files encountered. Returns tuples of (path, heading_summary) where
/// heading_summary is a comma-separated list of top-level headings. Results are deduplicated
/// and ordered with closest-to-file first.
pub fn resolve_agents_md(project_root: &Path, file_scope: &[PathBuf]) -> Result<Vec<(String, String)>> {
    let mut summaries: Vec<(String, String)> = Vec::new();
    let mut seen_paths: HashSet<PathBuf> = HashSet::new();

    // For each file in scope, walk from project root to the file
    for file_path in file_scope {
        // Normalize the file path relative to project root
        let absolute_path = if file_path.is_absolute() {
            file_path.clone()
        } else {
            project_root.join(file_path)
        };

        // Walk from project root to the file's parent, collecting AGENTS.md files
        let mut current = project_root.to_path_buf();

        // Collect all directories from root to file's parent
        let mut dirs_to_check = vec![current.clone()];

        loop {
            if let Some(parent) = absolute_path.parent() {
                if parent != current && current.starts_with(project_root) {
                    current = parent.to_path_buf();
                    dirs_to_check.push(current.clone());
                } else {
                    break;
                }
            } else {
                break;
            }

            if current == project_root {
                break;
            }
        }

        // Check each directory for AGENTS.md (reverse order: closest to file first)
        for dir in dirs_to_check.iter().rev() {
            let agents_md_path = dir.join("AGENTS.md");
            if agents_md_path.exists() && !seen_paths.contains(&agents_md_path) {
                seen_paths.insert(agents_md_path.clone());
                let headings = extract_headings(&agents_md_path)?;
                let heading_summary = headings.join(", ");
                let path_str = agents_md_path.to_string_lossy().to_string();
                summaries.push((path_str, heading_summary));
            }
        }
    }

    Ok(summaries)
}

/// Extract top-level headings (lines starting with "# ") from a markdown file
fn extract_headings(path: &Path) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(path)?;
    let mut headings = Vec::new();

    for line in content.lines() {
        if let Some(heading) = line.strip_prefix("# ") {
            headings.push(heading.trim().to_string());
        }
    }

    Ok(headings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_extract_headings() -> Result<()> {
        let tmpdir = TempDir::new()?;
        let agents_md_path = tmpdir.path().join("AGENTS.md");
        fs::write(
            &agents_md_path,
            "# Introduction\n# Getting Started\n## Subsection\n# Advanced",
        )?;

        let headings = extract_headings(&agents_md_path)?;
        assert_eq!(headings, vec!["Introduction", "Getting Started", "Advanced"]);
        Ok(())
    }

    #[test]
    fn test_resolve_agents_md_single_file() -> Result<()> {
        let tmpdir = TempDir::new()?;
        let project_root = tmpdir.path();

        // Create AGENTS.md at root
        fs::write(
            project_root.join("AGENTS.md"),
            "# Root\n# Guidelines",
        )?;

        // Create a file to scope
        fs::write(project_root.join("main.rs"), "fn main() {}")?;

        let summaries = resolve_agents_md(project_root, &[PathBuf::from("main.rs")])?;
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].1, "Root, Guidelines");
        Ok(())
    }

    #[test]
    fn test_resolve_agents_md_hierarchy() -> Result<()> {
        let tmpdir = TempDir::new()?;
        let project_root = tmpdir.path();

        // Create root AGENTS.md
        fs::write(project_root.join("AGENTS.md"), "# Root Guidelines")?;

        // Create src directory with AGENTS.md
        fs::create_dir(project_root.join("src"))?;
        fs::write(
            project_root.join("src/AGENTS.md"),
            "# Rust Guidelines",
        )?;

        // Create a file in src
        fs::write(project_root.join("src/main.rs"), "fn main() {}")?;

        let summaries = resolve_agents_md(project_root, &[PathBuf::from("src/main.rs")])?;

        // Should have both files, with src/AGENTS.md first (closest to file)
        assert_eq!(summaries.len(), 2);
        assert!(summaries[0].0.contains("src/AGENTS.md"));
        assert!(summaries[1].0.contains("AGENTS.md"));
        Ok(())
    }

    #[test]
    fn test_resolve_agents_md_deduplication() -> Result<()> {
        let tmpdir = TempDir::new()?;
        let project_root = tmpdir.path();

        // Create root AGENTS.md
        fs::write(project_root.join("AGENTS.md"), "# Root Guidelines")?;

        // Create two files in root
        fs::write(project_root.join("file1.rs"), "fn main() {}")?;
        fs::write(project_root.join("file2.rs"), "fn main() {}")?;

        let summaries = resolve_agents_md(
            project_root,
            &[PathBuf::from("file1.rs"), PathBuf::from("file2.rs")],
        )?;

        // Should have AGENTS.md only once despite two files in scope
        assert_eq!(summaries.len(), 1);
        Ok(())
    }
}
