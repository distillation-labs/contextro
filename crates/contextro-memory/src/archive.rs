//! Compaction archive for session context recovery.

/// Stores pre-compaction session context for searchable recovery.
pub struct CompactionArchive;

impl CompactionArchive {
    pub fn new() -> Self {
        Self
    }

    pub fn archive(&self, _content: &str) -> String {
        "arc_placeholder".into()
    }

    pub fn search(&self, _query: &str) -> Vec<String> {
        vec![]
    }
}

impl Default for CompactionArchive {
    fn default() -> Self {
        Self::new()
    }
}
