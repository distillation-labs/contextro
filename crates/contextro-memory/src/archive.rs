//! Compaction archive for session context recovery.
//!
//! Stores pre-compaction session context with TTL-based expiry.
//! Content can be searched via substring matching or retrieved by reference ID.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use sha2::{Digest, Sha256};

/// A single archived entry.
struct ArchiveEntry {
    content: String,
    _metadata: serde_json::Value,
    created: Instant,
    chars: usize,
}

/// Stores pre-compaction session context for searchable recovery.
pub struct CompactionArchive {
    entries: Mutex<BTreeMap<String, ArchiveEntry>>,
    max_entries: usize,
    ttl: Duration,
}

impl CompactionArchive {
    pub fn new() -> Self {
        Self::with_config(20, Duration::from_secs(86400))
    }

    pub fn with_config(max_entries: usize, ttl: Duration) -> Self {
        Self {
            entries: Mutex::new(BTreeMap::new()),
            max_entries,
            ttl,
        }
    }

    /// Archive pre-compaction content. Returns a reference ID.
    pub fn archive(&self, content: &str, metadata: Option<serde_json::Value>) -> String {
        let ref_id = Self::generate_id(content);
        let mut entries = self.entries.lock();

        // Prune expired
        let now = Instant::now();
        entries.retain(|_, e| now.duration_since(e.created) < self.ttl);

        // Evict oldest if at capacity
        while entries.len() >= self.max_entries {
            let oldest_key = entries.keys().next().cloned();
            if let Some(k) = oldest_key {
                entries.remove(&k);
            } else {
                break;
            }
        }

        entries.insert(
            ref_id.clone(),
            ArchiveEntry {
                content: content.to_string(),
                _metadata: metadata.unwrap_or(serde_json::Value::Null),
                created: Instant::now(),
                chars: content.len(),
            },
        );

        ref_id
    }

    /// Search archived content by substring. Returns matching excerpts.
    pub fn search(&self, query: &str, limit: usize) -> Vec<ArchiveSearchResult> {
        if query.is_empty() {
            return vec![];
        }
        let query_lower = query.to_lowercase();
        let entries = self.entries.lock();
        let mut results = Vec::new();

        for (ref_id, entry) in entries.iter().rev() {
            let content_lower = entry.content.to_lowercase();
            if let Some(pos) = content_lower.find(&query_lower) {
                let start = pos.saturating_sub(100);
                let end = (pos + query.len() + 100).min(entry.content.len());
                let excerpt = &entry.content[start..end];
                results.push(ArchiveSearchResult {
                    ref_id: ref_id.clone(),
                    excerpt: excerpt.to_string(),
                    chars: entry.chars,
                });
                if results.len() >= limit {
                    break;
                }
            }
        }
        results
    }

    /// Retrieve full content by reference ID.
    pub fn retrieve(&self, ref_id: &str) -> Option<String> {
        let entries = self.entries.lock();
        entries.get(ref_id).map(|e| e.content.clone())
    }

    /// Number of archived entries.
    pub fn len(&self) -> usize {
        self.entries.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.lock().is_empty()
    }

    fn generate_id(content: &str) -> String {
        let hash = Sha256::digest(content.as_bytes());
        format!("arc_{}", hex::encode(&hash[..4]))
    }
}

impl Default for CompactionArchive {
    fn default() -> Self {
        Self::new()
    }
}

/// A search result from the compaction archive.
#[derive(Debug, Clone)]
pub struct ArchiveSearchResult {
    pub ref_id: String,
    pub excerpt: String,
    pub chars: usize,
}

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_and_retrieve() {
        let archive = CompactionArchive::new();
        let ref_id = archive.archive("This is session content about JWT tokens", None);
        assert!(ref_id.starts_with("arc_"));

        let content = archive.retrieve(&ref_id).unwrap();
        assert!(content.contains("JWT tokens"));
    }

    #[test]
    fn test_search() {
        let archive = CompactionArchive::new();
        archive.archive("We decided to use Redis for caching", None);
        archive.archive("Authentication uses JWT with 24h expiry", None);

        let results = archive.search("JWT", 5);
        assert_eq!(results.len(), 1);
        assert!(results[0].excerpt.contains("JWT"));
    }

    #[test]
    fn test_eviction() {
        let archive = CompactionArchive::with_config(2, Duration::from_secs(86400));
        archive.archive("first", None);
        archive.archive("second", None);
        archive.archive("third", None);
        assert_eq!(archive.len(), 2);
    }
}
