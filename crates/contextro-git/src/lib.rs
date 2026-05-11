//! Git integration: commit indexing, branch watching, cross-repo.

pub mod branch_watcher;
pub mod commit_indexer;
pub mod cross_repo;

pub use commit_indexer::{is_git_repo, current_branch, head_hash};
