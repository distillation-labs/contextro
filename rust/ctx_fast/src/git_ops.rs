//! Git operations via command-line git (fast subprocess).
//!
//! Uses std::process::Command for git operations. While subprocess has
//! some overhead vs libgit2, it avoids compiling the large libgit2 C library
//! and works with any git version installed on the system.

use std::process::Command;

/// Check if a path is inside a git repository.
pub fn is_repo(path: &str) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get the current branch name.
pub fn current_branch(repo_path: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Get the HEAD commit hash (full 40-char hex).
pub fn head_hash(repo_path: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Get files changed between two commits.
pub fn changed_files(
    repo_path: &str,
    from_commit: Option<&str>,
    to_commit: Option<&str>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let range = match (from_commit, to_commit) {
        (Some(from), Some(to)) => format!("{}..{}", from, to),
        (Some(from), None) => format!("{}..HEAD", from),
        (None, Some(to)) => format!("HEAD~1..{}", to),
        (None, None) => "HEAD~1..HEAD".to_string(),
    };

    let output = Command::new("git")
        .args(["diff", "--name-only", &range])
        .current_dir(repo_path)
        .output()?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();

    Ok(files)
}

/// Get uncommitted changes (working directory status).
pub fn status_files(repo_path: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["status", "--porcelain", "-uall"])
        .current_dir(repo_path)
        .output()?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| l.len() > 3)
        .map(|l| l[3..].to_string())
        .collect();

    Ok(files)
}
