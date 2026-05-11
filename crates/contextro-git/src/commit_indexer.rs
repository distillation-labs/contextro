//! Git commit indexing using libgit2.

use std::path::Path;

/// Check if a path is inside a git repository.
pub fn is_git_repo(path: &str) -> bool {
    git2::Repository::discover(path).is_ok()
}

/// Get the current branch name.
pub fn current_branch(repo_path: &str) -> Option<String> {
    let repo = git2::Repository::discover(repo_path).ok()?;
    let head = repo.head().ok()?;
    head.shorthand().map(|s| s.to_string())
}

/// Get the current HEAD commit hash.
pub fn head_hash(repo_path: &str) -> Option<String> {
    let repo = git2::Repository::discover(repo_path).ok()?;
    let head = repo.head().ok()?;
    head.target().map(|oid| oid.to_string())
}

/// Get files changed between two commits.
pub fn changed_files(repo_path: &str, from: Option<&str>, to: Option<&str>) -> Vec<String> {
    let repo = match git2::Repository::discover(repo_path) {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    let to_commit = match to {
        Some(rev) => repo
            .revparse_single(rev)
            .ok()
            .and_then(|o| o.into_commit().ok()),
        None => repo.head().ok().and_then(|h| h.peel_to_commit().ok()),
    };

    let from_commit = match from {
        Some(rev) => repo
            .revparse_single(rev)
            .ok()
            .and_then(|o| o.into_commit().ok()),
        None => to_commit.as_ref().and_then(|c| c.parent(0).ok()),
    };

    let (from_commit, to_commit) = match (from_commit, to_commit) {
        (Some(f), Some(t)) => (f, t),
        _ => return vec![],
    };

    let from_tree = match from_commit.tree() {
        Ok(t) => t,
        Err(_) => return vec![],
    };
    let to_tree = match to_commit.tree() {
        Ok(t) => t,
        Err(_) => return vec![],
    };

    let diff = match repo.diff_tree_to_tree(Some(&from_tree), Some(&to_tree), None) {
        Ok(d) => d,
        Err(_) => return vec![],
    };

    let mut files = Vec::new();
    diff.foreach(
        &mut |delta, _| {
            if let Some(path) = delta.new_file().path() {
                files.push(path.to_string_lossy().to_string());
            }
            true
        },
        None,
        None,
        None,
    )
    .ok();

    files
}
