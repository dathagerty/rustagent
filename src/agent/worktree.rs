use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Manages git worktrees for parallel worker isolation.
///
/// Each work package gets its own git worktree branched from the goal branch.
/// After worker completion, worktree branches are merged back into the goal branch.
pub struct WorktreeManager {
    project_path: PathBuf,
}

impl WorktreeManager {
    pub fn new(project_path: PathBuf) -> Self {
        Self { project_path }
    }

    /// Get the project path.
    pub fn project_path(&self) -> &Path {
        &self.project_path
    }

    /// Create the goal branch from current HEAD. If branch already exists, reuse it.
    pub fn create_goal_branch(&self, goal_id: &str) -> Result<String> {
        let branch_name = format!("rustagent/{}", goal_id);

        // Ensure .gitignore entry exists
        self.ensure_gitignore()?;

        let output = Command::new("git")
            .args(["branch", &branch_name, "HEAD"])
            .current_dir(&self.project_path)
            .output()
            .context("failed to run git branch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("already exists") {
                return Ok(branch_name);
            }
            return Err(anyhow!("failed to create goal branch: {}", stderr));
        }
        Ok(branch_name)
    }

    /// Create a worktree for a work package.
    /// Returns the path to the worktree directory.
    pub fn create_worktree(&self, goal_id: &str, work_package_id: &str) -> Result<PathBuf> {
        let branch_name = format!("rustagent/{}-wp-{}", goal_id, work_package_id);
        let goal_branch = format!("rustagent/{}", goal_id);

        let worktree_path = self
            .project_path
            .join(".rustagent")
            .join("worktrees")
            .join(format!("{}-wp-{}", goal_id, work_package_id));

        // Create parent directory
        if let Some(parent) = worktree_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let output = Command::new("git")
            .args([
                "worktree",
                "add",
                "-b",
                &branch_name,
                worktree_path
                    .to_str()
                    .ok_or_else(|| anyhow!("invalid worktree path"))?,
                &goal_branch,
            ])
            .current_dir(&self.project_path)
            .output()
            .context("failed to run git worktree add")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("failed to create worktree: {}", stderr));
        }

        Ok(worktree_path)
    }

    /// Merge a work package branch into the goal branch.
    ///
    /// Uses a temporary merge worktree to avoid depending on the main
    /// worktree's current branch state.
    pub fn merge_work_package(&self, goal_id: &str, work_package_id: &str) -> Result<()> {
        let branch_name = format!("rustagent/{}-wp-{}", goal_id, work_package_id);
        let goal_branch = format!("rustagent/{}", goal_id);

        let merge_path = self
            .project_path
            .join(".rustagent")
            .join("worktrees")
            .join(format!("{}-merge-tmp", goal_id));

        // Clean up any leftover merge worktree
        if merge_path.exists() {
            let _ = Command::new("git")
                .args([
                    "worktree",
                    "remove",
                    "--force",
                    merge_path.to_str().unwrap_or_default(),
                ])
                .current_dir(&self.project_path)
                .output();
        }

        // Create temporary worktree on goal branch
        let output = Command::new("git")
            .args([
                "worktree",
                "add",
                merge_path
                    .to_str()
                    .ok_or_else(|| anyhow!("invalid merge path"))?,
                &goal_branch,
            ])
            .current_dir(&self.project_path)
            .output()
            .context("failed to create merge worktree")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("failed to create merge worktree: {}", stderr));
        }

        // Merge the work package branch into goal branch
        let merge_result = Command::new("git")
            .args([
                "merge",
                "--no-ff",
                "-m",
                &format!("rustagent: merge work package wp-{}", work_package_id),
                &branch_name,
            ])
            .current_dir(&merge_path)
            .output()
            .context("failed to run git merge")?;

        // Clean up merge worktree regardless of outcome
        let _ = Command::new("git")
            .args([
                "worktree",
                "remove",
                "--force",
                merge_path.to_str().unwrap_or_default(),
            ])
            .current_dir(&self.project_path)
            .output();

        if !merge_result.status.success() {
            let stderr = String::from_utf8_lossy(&merge_result.stderr);
            return Err(anyhow!("merge conflict in work package branch: {}", stderr));
        }

        Ok(())
    }

    /// Remove a worktree and its branch after successful merge.
    pub fn cleanup_worktree(&self, goal_id: &str, work_package_id: &str) -> Result<()> {
        let branch_name = format!("rustagent/{}-wp-{}", goal_id, work_package_id);
        let worktree_path = self
            .project_path
            .join(".rustagent")
            .join("worktrees")
            .join(format!("{}-wp-{}", goal_id, work_package_id));

        // Remove worktree
        if worktree_path.exists() {
            let _ = Command::new("git")
                .args([
                    "worktree",
                    "remove",
                    "--force",
                    worktree_path.to_str().unwrap_or_default(),
                ])
                .current_dir(&self.project_path)
                .output();
        }

        // Delete branch
        let _ = Command::new("git")
            .args(["branch", "-D", &branch_name])
            .current_dir(&self.project_path)
            .output();

        Ok(())
    }

    /// Get the goal branch name for a goal ID.
    pub fn goal_branch_name(goal_id: &str) -> String {
        format!("rustagent/{}", goal_id)
    }

    /// Ensure `.rustagent/worktrees/` is in `.gitignore`.
    fn ensure_gitignore(&self) -> Result<()> {
        let gitignore_path = self.project_path.join(".gitignore");
        let entry = ".rustagent/worktrees/";

        if gitignore_path.exists() {
            let content = std::fs::read_to_string(&gitignore_path)?;
            if content.contains(entry) {
                return Ok(());
            }
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&gitignore_path)?;
            use std::io::Write;
            writeln!(file, "\n# Rustagent worktrees (auto-generated)")?;
            writeln!(file, "{}", entry)?;
        } else {
            std::fs::write(
                &gitignore_path,
                format!("# Rustagent worktrees (auto-generated)\n{}\n", entry),
            )?;
        }
        Ok(())
    }
}
