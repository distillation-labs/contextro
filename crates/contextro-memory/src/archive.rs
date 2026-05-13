//! Compaction archive for session context recovery.
//!
//! Stores pre-compaction session context with TTL-based expiry.
//! Content is persisted to disk so refs survive stdio process restarts.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use contextro_config::get_settings;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A single archived entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ArchiveEntry {
    content: String,
    metadata: serde_json::Value,
    created_at: i64,
    chars: usize,
}

/// Stores pre-compaction session context for searchable recovery.
pub struct CompactionArchive {
    entries: Mutex<BTreeMap<String, ArchiveEntry>>,
    max_entries: usize,
    ttl: Duration,
    file_path: PathBuf,
}

impl CompactionArchive {
    pub fn new() -> Self {
        Self::with_config(20, Duration::from_secs(86400))
    }

    pub fn with_config(max_entries: usize, ttl: Duration) -> Self {
        let storage_dir = get_settings().read().storage_dir.clone();
        Self::with_path(
            PathBuf::from(storage_dir).join("session-archive.json"),
            max_entries,
            ttl,
        )
    }

    pub fn with_path<P: Into<PathBuf>>(file_path: P, max_entries: usize, ttl: Duration) -> Self {
        let file_path = file_path.into();
        let entries = load_entries(&file_path);
        Self {
            entries: Mutex::new(entries),
            max_entries,
            ttl,
            file_path,
        }
    }

    /// Archive pre-compaction content. Returns a reference ID.
    pub fn archive(&self, content: &str, metadata: Option<serde_json::Value>) -> String {
        let ref_id = Self::generate_id(content);
        let mut entries = self.entries.lock();
        self.prune_locked(&mut entries);

        while entries.len() >= self.max_entries {
            self.remove_oldest_locked(&mut entries);
        }

        entries.insert(
            ref_id.clone(),
            ArchiveEntry {
                content: content.to_string(),
                metadata: metadata.unwrap_or(serde_json::Value::Null),
                created_at: Utc::now().timestamp(),
                chars: content.len(),
            },
        );
        self.save_locked(&entries);

        ref_id
    }

    /// Search archived content by substring. Returns matching excerpts.
    pub fn search(&self, query: &str, limit: usize) -> Vec<ArchiveSearchResult> {
        if query.is_empty() {
            return vec![];
        }
        let query_lower = query.to_lowercase();
        let mut entries = self.entries.lock();
        self.prune_locked(&mut entries);
        self.save_locked(&entries);

        let mut matches: Vec<(i64, ArchiveSearchResult)> = entries
            .iter()
            .filter_map(|(ref_id, entry)| {
                let content_lower = entry.content.to_lowercase();
                let pos = content_lower.find(&query_lower)?;
                let start = pos.saturating_sub(100);
                let end = (pos + query.len() + 100).min(entry.content.len());
                let excerpt = &entry.content[start..end];
                Some((
                    entry.created_at,
                    ArchiveSearchResult {
                        ref_id: ref_id.clone(),
                        excerpt: excerpt.to_string(),
                        chars: entry.chars,
                    },
                ))
            })
            .collect();

        matches.sort_by_key(|(created_at, _)| std::cmp::Reverse(*created_at));
        matches
            .into_iter()
            .take(limit)
            .map(|(_, result)| result)
            .collect()
    }

    /// Retrieve full content by reference ID.
    pub fn retrieve(&self, ref_id: &str) -> Option<String> {
        let mut entries = self.entries.lock();
        self.prune_locked(&mut entries);
        self.save_locked(&entries);
        entries.get(ref_id).map(|entry| entry.content.clone())
    }

    /// Number of archived entries.
    pub fn len(&self) -> usize {
        let mut entries = self.entries.lock();
        self.prune_locked(&mut entries);
        self.save_locked(&entries);
        entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn generate_id(content: &str) -> String {
        let hash = Sha256::digest(content.as_bytes());
        format!("arc_{}", hex::encode(&hash[..4]))
    }

    fn prune_locked(&self, entries: &mut BTreeMap<String, ArchiveEntry>) {
        let cutoff = Utc::now().timestamp() - self.ttl.as_secs() as i64;
        entries.retain(|_, entry| entry.created_at >= cutoff);
    }

    fn remove_oldest_locked(&self, entries: &mut BTreeMap<String, ArchiveEntry>) {
        let oldest_ref = entries
            .iter()
            .min_by_key(|(_, entry)| entry.created_at)
            .map(|(ref_id, _)| ref_id.clone());
        if let Some(ref_id) = oldest_ref {
            entries.remove(&ref_id);
        }
    }

    fn save_locked(&self, entries: &BTreeMap<String, ArchiveEntry>) {
        if let Some(parent) = self.file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp_path = self.file_path.with_extension("json.tmp");
        if let Ok(bytes) = serde_json::to_vec_pretty(entries) {
            if std::fs::write(&tmp_path, bytes).is_ok() {
                let _ = std::fs::rename(&tmp_path, &self.file_path);
            }
        }
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

fn load_entries(path: &Path) -> BTreeMap<String, ArchiveEntry> {
    let Ok(bytes) = std::fs::read(path) else {
        return BTreeMap::new();
    };

    match serde_json::from_slice::<BTreeMap<String, ArchiveEntry>>(&bytes) {
        Ok(entries) => entries,
        Err(_) => {
            backup_corrupt_file(path);
            BTreeMap::new()
        }
    }
}

fn backup_corrupt_file(path: &Path) {
    let backup = path.with_extension(format!("corrupt-{}.json", Utc::now().timestamp()));
    let _ = std::fs::rename(path, backup);
}

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("contextro-archive-{unique}-{name}"))
    }

    #[test]
    fn test_archive_and_retrieve() {
        let path = temp_file("archive.json");
        let archive = CompactionArchive::with_path(&path, 20, Duration::from_secs(86400));
        let ref_id = archive.archive("This is session content about JWT tokens", None);
        assert!(ref_id.starts_with("arc_"));

        let content = archive.retrieve(&ref_id).unwrap();
        assert!(content.contains("JWT tokens"));

        let reloaded = CompactionArchive::with_path(&path, 20, Duration::from_secs(86400));
        let reloaded_content = reloaded.retrieve(&ref_id).unwrap();
        assert!(reloaded_content.contains("JWT tokens"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_search() {
        let path = temp_file("search.json");
        let archive = CompactionArchive::with_path(&path, 20, Duration::from_secs(86400));
        archive.archive("We decided to use Redis for caching", None);
        archive.archive("Authentication uses JWT with 24h expiry", None);

        let results = archive.search("JWT", 5);
        assert_eq!(results.len(), 1);
        assert!(results[0].excerpt.contains("JWT"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_eviction() {
        let path = temp_file("eviction.json");
        let archive = CompactionArchive::with_path(&path, 2, Duration::from_secs(86400));
        archive.archive("first", None);
        archive.archive("second", None);
        archive.archive("third", None);
        assert_eq!(archive.len(), 2);

        let _ = std::fs::remove_file(path);
    }
}
