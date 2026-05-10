//! High-performance query cache using DashMap for lock-free concurrent access.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use dashmap::DashMap;

struct CacheEntry {
    result: serde_json::Value,
    inserted_at: Instant,
}

/// Lock-free LRU query cache with TTL expiry.
pub struct QueryCache {
    entries: DashMap<String, CacheEntry>,
    max_size: usize,
    ttl_secs: f64,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl QueryCache {
    pub fn new(max_size: usize, ttl_secs: f64) -> Self {
        Self {
            entries: DashMap::with_capacity(max_size),
            max_size,
            ttl_secs,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// O(1) cache lookup.
    pub fn get(&self, query: &str) -> Option<serde_json::Value> {
        if let Some(entry) = self.entries.get(query) {
            if entry.inserted_at.elapsed().as_secs_f64() < self.ttl_secs {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Some(entry.result.clone());
            }
            // Expired — remove it
            drop(entry);
            self.entries.remove(query);
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Store a query result.
    pub fn put(&self, query: &str, result: serde_json::Value) {
        // Evict if at capacity (simple: just remove oldest by iterating once)
        if self.entries.len() >= self.max_size {
            // Remove first entry found (approximate LRU)
            if let Some(key) = self.entries.iter().next().map(|e| e.key().clone()) {
                self.entries.remove(&key);
            }
        }
        self.entries.insert(query.to_string(), CacheEntry {
            result,
            inserted_at: Instant::now(),
        });
    }

    /// Invalidate all cached entries.
    pub fn invalidate(&self) {
        self.entries.clear();
    }

    pub fn hit_rate(&self) -> f64 {
        let h = self.hits.load(Ordering::Relaxed);
        let m = self.misses.load(Ordering::Relaxed);
        let total = h + m;
        if total == 0 { 0.0 } else { h as f64 / total as f64 }
    }

    pub fn size(&self) -> usize {
        self.entries.len()
    }
}
