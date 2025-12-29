//! Gog CLI wrapper for QwenCoder Bot
//!
//! Provides interface to interact with Gogs via the `gog` CLI tool.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Wrapper for the gog CLI tool
pub struct GogClient {
    gog_binary: PathBuf,
    timeout: Duration,
}

/// Issue data from Gogs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub state: String,
    pub labels: Vec<Label>,
    pub repo: String,
    pub owner: String,
    #[serde(default)]
    pub assignees: Vec<String>,
    #[serde(default)]
    pub comments: Vec<Comment>,
}

/// Label data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    #[serde(default)]
    pub color: String,
}

/// Comment data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: u64,
    pub body: String,
    pub user: String,
    #[serde(default)]
    pub created_at: String,
}

/// Repository data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub name: String,
    pub full_name: String,
    pub owner: String,
    pub clone_url: String,
    pub ssh_url: String,
    #[serde(default)]
    pub default_branch: String,
    #[serde(default)]
    pub description: String,
}

impl GogClient {
    /// Create a new Gog client
    pub fn new(gog_binary: Option<PathBuf>, timeout_secs: u64) -> Self {
        let gog_binary = gog_binary.unwrap_or_else(|| PathBuf::from("gog"));

        Self {
            gog_binary,
            timeout: Duration::from_secs(timeout_secs),
        }
    }

    /// Execute a gog command with a specific profile
    fn exec(&self, profile: &str, args: &[&str]) -> Result<String> {
        debug!("Executing: gog --profile {} {}", profile, args.join(" "));

        let output = Command::new(&self.gog_binary)
            .arg("--profile")
            .arg(profile)
            .args(args)
            .output()
            .with_context(|| format!("Failed to execute gog command: {:?}", args))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("gog command failed: {}", stderr);
            anyhow::bail!("gog command failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    }

    /// List issues with a specific label
    pub fn list_issues_by_label(
        &self,
        profile: &str,
        label: &str,
        repo: Option<&str>,
    ) -> Result<Vec<Issue>> {
        let mut args = vec!["issue", "list", "--label", label, "--json"];

        if let Some(r) = repo {
            args.push("--repo");
            args.push(r);
        }

        let output = self.exec(profile, &args)?;

        if output.trim().is_empty() {
            return Ok(Vec::new());
        }

        let issues: Vec<Issue> = serde_json::from_str(&output)
            .with_context(|| "Failed to parse issue list")?;

        Ok(issues)
    }

    /// Get details for a specific issue
    pub fn get_issue(
        &self,
        profile: &str,
        repo: &str,
        issue_number: u64,
    ) -> Result<Issue> {
        let num_str = issue_number.to_string();
        let output = self.exec(
            profile,
            &["issue", "show", &num_str, "--repo", repo, "--json"],
        )?;

        let issue: Issue = serde_json::from_str(&output)
            .with_context(|| format!("Failed to parse issue #{}", issue_number))?;

        Ok(issue)
    }

    /// Add a comment to an issue
    pub fn add_comment(
        &self,
        profile: &str,
        repo: &str,
        issue_number: u64,
        body: &str,
    ) -> Result<()> {
        let num_str = issue_number.to_string();

        // Write body to temp file for long comments
        let temp_file = std::env::temp_dir().join(format!("gog-comment-{}.txt", issue_number));
        std::fs::write(&temp_file, body)?;

        let result = self.exec(
            profile,
            &[
                "issue", "comment", &num_str,
                "--repo", repo,
                "--body-file", temp_file.to_str().unwrap(),
            ],
        );

        // Clean up temp file
        let _ = std::fs::remove_file(&temp_file);

        result?;
        info!("Added comment to issue #{}", issue_number);
        Ok(())
    }

    /// Add a label to an issue
    pub fn add_label(
        &self,
        profile: &str,
        repo: &str,
        issue_number: u64,
        label: &str,
    ) -> Result<()> {
        let num_str = issue_number.to_string();

        self.exec(
            profile,
            &["issue", "label", &num_str, "--repo", repo, "--add", label],
        )?;

        info!("Added label '{}' to issue #{}", label, issue_number);
        Ok(())
    }

    /// Remove a label from an issue
    pub fn remove_label(
        &self,
        profile: &str,
        repo: &str,
        issue_number: u64,
        label: &str,
    ) -> Result<()> {
        let num_str = issue_number.to_string();

        self.exec(
            profile,
            &["issue", "label", &num_str, "--repo", repo, "--remove", label],
        )?;

        info!("Removed label '{}' from issue #{}", label, issue_number);
        Ok(())
    }

    /// List accessible repositories
    pub fn list_repos(&self, profile: &str) -> Result<Vec<Repository>> {
        let output = self.exec(profile, &["repo", "list", "--json"])?;

        if output.trim().is_empty() {
            return Ok(Vec::new());
        }

        let repos: Vec<Repository> = serde_json::from_str(&output)
            .with_context(|| "Failed to parse repository list")?;

        Ok(repos)
    }

    /// Get repository details
    pub fn get_repo(&self, profile: &str, repo: &str) -> Result<Repository> {
        let output = self.exec(profile, &["repo", "show", repo, "--json"])?;

        let repo: Repository = serde_json::from_str(&output)
            .with_context(|| format!("Failed to parse repository: {}", repo))?;

        Ok(repo)
    }

    /// Clone or update a repository
    pub fn clone_or_update(
        &self,
        profile: &str,
        repo: &str,
        target_dir: &std::path::Path,
    ) -> Result<()> {
        if target_dir.exists() {
            // Update existing clone
            info!("Updating existing clone at {:?}", target_dir);

            let output = Command::new("git")
                .current_dir(target_dir)
                .args(["fetch", "--all"])
                .output()
                .context("Failed to fetch updates")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Git fetch warning: {}", stderr);
            }

            let output = Command::new("git")
                .current_dir(target_dir)
                .args(["reset", "--hard", "origin/HEAD"])
                .output()
                .context("Failed to reset to origin")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Git reset failed: {}", stderr);
            }
        } else {
            // Fresh clone
            info!("Cloning {} to {:?}", repo, target_dir);

            let repo_info = self.get_repo(profile, repo)?;
            let clone_url = &repo_info.clone_url;

            let output = Command::new("git")
                .args(["clone", clone_url, target_dir.to_str().unwrap()])
                .output()
                .context("Failed to clone repository")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Git clone failed: {}", stderr);
            }
        }

        Ok(())
    }

    /// Create a branch in a repository
    pub fn create_branch(
        &self,
        repo_dir: &std::path::Path,
        branch_name: &str,
    ) -> Result<()> {
        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["checkout", "-b", branch_name])
            .output()
            .context("Failed to create branch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to create branch: {}", stderr);
        }

        info!("Created branch: {}", branch_name);
        Ok(())
    }

    /// Commit changes
    pub fn commit(&self, repo_dir: &std::path::Path, message: &str) -> Result<()> {
        // Stage all changes
        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["add", "-A"])
            .output()
            .context("Failed to stage changes")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to stage changes: {}", stderr);
        }

        // Commit
        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["commit", "-m", message])
            .output()
            .context("Failed to commit")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to commit: {}", stderr);
        }

        info!("Committed changes");
        Ok(())
    }

    /// Push a branch to origin
    pub fn push_branch(
        &self,
        repo_dir: &std::path::Path,
        branch_name: &str,
    ) -> Result<()> {
        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["push", "-u", "origin", branch_name])
            .output()
            .context("Failed to push branch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to push: {}", stderr);
        }

        info!("Pushed branch: {}", branch_name);
        Ok(())
    }

    /// Get file contents from a repository
    pub fn read_file(&self, repo_dir: &std::path::Path, file_path: &str) -> Result<String> {
        let full_path = repo_dir.join(file_path);

        if !full_path.exists() {
            anyhow::bail!("File not found: {}", file_path);
        }

        let contents = std::fs::read_to_string(&full_path)
            .with_context(|| format!("Failed to read file: {}", file_path))?;

        Ok(contents)
    }

    /// List files in a repository (optionally filtered by pattern)
    pub fn list_files(
        &self,
        repo_dir: &std::path::Path,
        pattern: Option<&str>,
    ) -> Result<Vec<String>> {
        let args = if let Some(p) = pattern {
            vec!["ls-files", p]
        } else {
            vec!["ls-files"]
        };

        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(&args)
            .output()
            .context("Failed to list files")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to list files: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let files: Vec<String> = stdout
            .lines()
            .map(|s| s.to_string())
            .collect();

        Ok(files)
    }

    /// Detect primary language of a repository
    pub fn detect_language(&self, repo_dir: &std::path::Path) -> Result<String> {
        let files = self.list_files(repo_dir, None)?;

        let mut rust_count = 0;
        let mut python_count = 0;
        let mut csharp_count = 0;
        let mut js_count = 0;
        let mut shell_count = 0;

        for file in &files {
            if file.ends_with(".rs") {
                rust_count += 1;
            } else if file.ends_with(".py") {
                python_count += 1;
            } else if file.ends_with(".cs") {
                csharp_count += 1;
            } else if file.ends_with(".js") || file.ends_with(".ts") {
                js_count += 1;
            } else if file.ends_with(".sh") || file.ends_with(".bash") {
                shell_count += 1;
            }
        }

        // Also check for project files
        if repo_dir.join("Cargo.toml").exists() {
            rust_count += 10;
        }
        if repo_dir.join("pyproject.toml").exists() || repo_dir.join("setup.py").exists() {
            python_count += 10;
        }
        if files.iter().any(|f| f.ends_with(".csproj") || f.ends_with(".sln")) {
            csharp_count += 10;
        }
        if repo_dir.join("package.json").exists() {
            js_count += 10;
        }

        let max = [
            (rust_count, "rust"),
            (python_count, "python"),
            (csharp_count, "csharp"),
            (js_count, "javascript"),
            (shell_count, "shell"),
        ]
        .into_iter()
        .max_by_key(|(count, _)| *count)
        .map(|(_, lang)| lang)
        .unwrap_or("unknown");

        Ok(max.to_string())
    }
}

/// Format a comment with the bot signature
pub fn format_comment(signature: &str, content: &str) -> String {
    format!("{}\n\n{}", signature, content)
}

/// Format an assessment comment
pub fn format_assessment_comment(
    decision: &str,
    confidence: u32,
    reasoning: &str,
    affected_files: &[String],
    test_strategy: &str,
) -> String {
    let mut comment = format!(
        "## Assessment: {}\n\n**Confidence**: {}%\n\n**Reasoning**:\n{}\n",
        decision, confidence, reasoning
    );

    if !affected_files.is_empty() {
        comment.push_str("\n**Files to modify**:\n");
        for file in affected_files {
            comment.push_str(&format!("- `{}`\n", file));
        }
    }

    if !test_strategy.is_empty() {
        comment.push_str(&format!("\n**Test strategy**:\n{}\n", test_strategy));
    }

    comment
}

/// Format an implementation success comment
pub fn format_success_comment(
    branch: &str,
    test_results: &[(String, bool)],
    changes: &[(String, String)],
) -> String {
    let mut comment = format!(
        "## Implementation Complete\n\n\
         **Branch**: `{}`\n\n\
         **Test Results**:\n",
        branch
    );

    for (test, passed) in test_results {
        let icon = if *passed { "PASS" } else { "FAIL" };
        comment.push_str(&format!("- `{}`: {}\n", test, icon));
    }

    if !changes.is_empty() {
        comment.push_str("\n**Changes**:\n");
        for (file, description) in changes {
            comment.push_str(&format!("- `{}`: {}\n", file, description));
        }
    }

    comment.push_str("\n**Ready for review.**");
    comment
}

/// Format an implementation failure comment
pub fn format_failure_comment(error: &str, logs: Option<&str>) -> String {
    let mut comment = format!(
        "## Implementation Failed\n\n\
         **Error**: {}\n",
        error
    );

    if let Some(log_content) = logs {
        comment.push_str(&format!(
            "\n**Logs**:\n```\n{}\n```\n",
            log_content
        ));
    }

    comment.push_str("\nEscalating to human developer.");
    comment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_assessment_comment() {
        let comment = format_assessment_comment(
            "worth-attempting",
            85,
            "This is a simple fix.",
            &["src/lib.rs".to_string()],
            "Run cargo test",
        );

        assert!(comment.contains("worth-attempting"));
        assert!(comment.contains("85%"));
        assert!(comment.contains("src/lib.rs"));
    }

    #[test]
    fn test_format_success_comment() {
        let comment = format_success_comment(
            "qwen-fix-42",
            &[("cargo test".to_string(), true)],
            &[("src/lib.rs".to_string(), "Fixed off-by-one".to_string())],
        );

        assert!(comment.contains("qwen-fix-42"));
        assert!(comment.contains("PASS"));
        assert!(comment.contains("Ready for review"));
    }
}
