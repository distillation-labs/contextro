//! SQLite-backed semantic memory store.

use contextro_core::models::{Memory, MemoryType};
use contextro_core::ContextroError;

/// Memory store with remember/recall/forget operations.
pub struct MemoryStore {
    // TODO: rusqlite connection
}

impl MemoryStore {
    pub fn new(_db_path: &str) -> Result<Self, ContextroError> {
        Ok(Self {})
    }

    pub fn remember(&self, _memory: Memory) -> Result<String, ContextroError> {
        // TODO: embed + store
        Ok("mem_placeholder".into())
    }

    pub fn recall(&self, _query: &str, _limit: usize) -> Result<Vec<Memory>, ContextroError> {
        Ok(vec![])
    }

    pub fn forget(&self, _id: &str) -> Result<(), ContextroError> {
        Ok(())
    }
}
