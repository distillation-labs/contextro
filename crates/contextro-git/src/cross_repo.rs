//! Cross-repo context management.

/// Manages multiple registered repositories.
pub struct CrossRepoManager;

impl CrossRepoManager {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CrossRepoManager {
    fn default() -> Self {
        Self::new()
    }
}
