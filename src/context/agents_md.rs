use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Resolve AGENTS.md files for a given scope of files
///
/// This walks the directory hierarchy from the project root towards each file in scope,
/// collecting all AGENTS.md files encountered. Returns tuples of (path, heading_summary) where
/// heading_summary is a comma-separated list of top-level headings. Results are deduplicated
/// and ordered with closest-to-file first.
pub fn resolve_agents_md(
    project_root: &Path,
    file_scope: &[PathBuf],
) -> Result<Vec<(String, String)>> {
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

        // Walk from the file's parent directory up to project root, collecting directories
        let file_parent = absolute_path.parent().unwrap_or(project_root);
        let mut current = file_parent.to_path_buf();
        let mut dirs_to_check = Vec::new();

        // Collect all directories from file parent up to project root
        while current.starts_with(project_root) {
            dirs_to_check.push(current.clone());
            if current == project_root {
                break;
            }
            match current.parent() {
                Some(p) => current = p.to_path_buf(),
                None => break,
            }
        }

        // Check each directory for AGENTS.md (closest to file first)
        for dir in &dirs_to_check {
            let agents_md_path = dir.join("AGENTS.md");
            if agents_md_path.exists() && !seen_paths.contains(&agents_md_path) {
                seen_paths.insert(agents_md_path.clone());
                let heading_summaries = extract_heading_summaries(&agents_md_path)?;
                let heading_summary = heading_summaries
                    .iter()
                    .map(|(heading, count)| format!("{} ({} lines)", heading, count))
                    .collect::<Vec<_>>()
                    .join(", ");
                let path_str = agents_md_path.to_string_lossy().to_string();
                summaries.push((path_str, heading_summary));
            }
        }
    }

    Ok(summaries)
}

/// Extract top-level headings with content line counts from a markdown file
///
/// Returns a vector of (heading_text, line_count) tuples, where line_count is the number
/// of non-empty content lines under that heading (until the next heading or EOF).
fn extract_heading_summaries(path: &Path) -> Result<Vec<(String, usize)>> {
    let content = std::fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut summaries = Vec::new();

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];

        // Check if this is a top-level heading (# followed by space)
        if let Some(heading) = line.strip_prefix("# ") {
            let heading_text = heading.trim().to_string();

            // Count non-empty lines until the next top-level heading or EOF
            let mut line_count = 0;
            let mut j = i + 1;
            while j < lines.len() {
                let next_line = lines[j];
                // Stop at the next top-level heading
                if next_line.starts_with("# ") {
                    break;
                }
                // Count non-empty lines
                if !next_line.trim().is_empty() {
                    line_count += 1;
                }
                j += 1;
            }

            summaries.push((heading_text, line_count));
        }

        i += 1;
    }

    Ok(summaries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_extract_heading_summaries() -> Result<()> {
        let tmpdir = TempDir::new()?;
        let agents_md_path = tmpdir.path().join("AGENTS.md");
        fs::write(
            &agents_md_path,
            "# Introduction\nSome content here\nMore content\n# Getting Started\n\n## Subsection\ndetails\n# Advanced",
        )?;

        let summaries = extract_heading_summaries(&agents_md_path)?;
        assert_eq!(summaries.len(), 3);
        assert_eq!(summaries[0].0, "Introduction");
        assert_eq!(summaries[0].1, 2); // "Some content here" and "More content"
        assert_eq!(summaries[1].0, "Getting Started");
        assert_eq!(summaries[1].1, 2); // "## Subsection" and "details" (both non-empty content lines)
        assert_eq!(summaries[2].0, "Advanced");
        assert_eq!(summaries[2].1, 0); // no content after
        Ok(())
    }

    #[test]
    fn test_extract_heading_summaries_empty_sections() -> Result<()> {
        let tmpdir = TempDir::new()?;
        let agents_md_path = tmpdir.path().join("AGENTS.md");
        fs::write(
            &agents_md_path,
            "# First\n\n\n# Second\nContent\n# Third\n",
        )?;

        let summaries = extract_heading_summaries(&agents_md_path)?;
        assert_eq!(summaries.len(), 3);
        assert_eq!(summaries[0].1, 0); // First has no content
        assert_eq!(summaries[1].1, 1); // Second has one line
        assert_eq!(summaries[2].1, 0); // Third has no content
        Ok(())
    }

    #[test]
    fn test_resolve_agents_md_single_file() -> Result<()> {
        let tmpdir = TempDir::new()?;
        let project_root = tmpdir.path();

        // Create AGENTS.md at root
        fs::write(project_root.join("AGENTS.md"), "# Root\nroot content\n# Guidelines")?;

        // Create a file to scope
        fs::write(project_root.join("main.rs"), "fn main() {}")?;

        let summaries = resolve_agents_md(project_root, &[PathBuf::from("main.rs")])?;
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].1, "Root (1 lines), Guidelines (0 lines)");
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
        fs::write(project_root.join("src/AGENTS.md"), "# Rust Guidelines")?;

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

    #[test]
    fn test_resolve_agents_md_nested_three_levels() -> Result<()> {
        let tmpdir = TempDir::new()?;
        let project_root = tmpdir.path();

        // Create AGENTS.md at root
        fs::write(project_root.join("AGENTS.md"), "# Root Guidelines")?;

        // Create src directory with AGENTS.md
        fs::create_dir(project_root.join("src"))?;
        fs::write(project_root.join("src/AGENTS.md"), "# Rust Guidelines")?;

        // Create src/auth directory with AGENTS.md
        fs::create_dir(project_root.join("src/auth"))?;
        fs::write(
            project_root.join("src/auth/AGENTS.md"),
            "# Auth Module Guidelines",
        )?;

        // Create a file deep in the hierarchy
        fs::write(project_root.join("src/auth/handler.rs"), "fn handle() {}")?;

        let summaries = resolve_agents_md(project_root, &[PathBuf::from("src/auth/handler.rs")])?;

        // Should have all three AGENTS.md files, in order: closest to file first
        assert_eq!(summaries.len(), 3);
        assert!(summaries[0].0.contains("src/auth/AGENTS.md"));
        assert!(summaries[1].0.contains("src/AGENTS.md"));
        assert!(summaries[2].0.contains("AGENTS.md"));
        Ok(())
    }
}
