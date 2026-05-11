//! Branch watcher for real-time reindexing.

/// Watches for branch changes and triggers reindexing.
pub struct BranchWatcher;

impl BranchWatcher {
    pub fn new() -> Self { Self }
    pub fn is_running(&self) -> bool { false }
}

impl Default for BranchWatcher {
    fn default() -> Self { Self::new() }
}
