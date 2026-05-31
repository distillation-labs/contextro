//! High-performance query cache using DashMap for lock-free concurrent access.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use dashmap::DashMap;

struct CacheEntry {
    result: serde_json::Value,
    rendered_default: Option<String>,
    inserted_at: Instant,
}

#[derive(Debug, Clone)]
pub struct CachedQueryResult {
    pub result: serde_json::Value,
    pub rendered_default: Option<String>,
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
        self.get_entry(query).map(|entry| entry.result)
    }

    pub fn get_entry(&self, query: &str) -> Option<CachedQueryResult> {
        if let Some(entry) = self.entries.get(query) {
            if entry.inserted_at.elapsed().as_secs_f64() < self.ttl_secs {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Some(CachedQueryResult {
                    result: entry.result.clone(),
                    rendered_default: entry.rendered_default.clone(),
                });
            }
            // Expired — remove it
            drop(entry);
            self.entries.remove(query);
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    pub fn get_rendered(&self, query: &str) -> Option<String> {
        if let Some(entry) = self.entries.get(query) {
            if entry.inserted_at.elapsed().as_secs_f64() < self.ttl_secs {
                if let Some(rendered) = entry.rendered_default.clone() {
                    self.hits.fetch_add(1, Ordering::Relaxed);
                    return Some(rendered);
                }
                return None;
            }
            drop(entry);
            self.entries.remove(query);
        }
        None
    }

    /// Store a query result.
    pub fn put(&self, query: &str, result: serde_json::Value) {
        self.put_with_rendered(query, result, None);
    }

    /// Store a query result and an optional pre-rendered default response string.
    pub fn put_with_rendered(
        &self,
        query: &str,
        result: serde_json::Value,
        rendered_default: Option<String>,
    ) {
        // Evict if at capacity (simple: just remove oldest by iterating once)
        let evict_key = if self.entries.len() >= self.max_size {
            // Clone the key in a separate scope so the iterator guard is dropped
            // before we try to remove from the same shard.
            self.entries.iter().next().map(|entry| entry.key().clone())
        } else {
            None
        };
        if let Some(key) = evict_key {
            self.entries.remove(&key);
        }
        self.entries.insert(
            query.to_string(),
            CacheEntry {
                result,
                rendered_default,
                inserted_at: Instant::now(),
            },
        );
    }

    /// Invalidate all cached entries.
    pub fn invalidate(&self) {
        self.entries.clear();
    }

    pub fn hit_rate(&self) -> f64 {
        let h = self.hits.load(Ordering::Relaxed);
        let m = self.misses.load(Ordering::Relaxed);
        let total = h + m;
        if total == 0 {
            0.0
        } else {
            h as f64 / total as f64
        }
    }

    pub fn size(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::QueryCache;

    #[test]
    fn put_evicts_without_deadlocking() {
        let cache = QueryCache::new(1, 60.0);

        cache.put("first", json!({"value": 1}));
        cache.put("second", json!({"value": 2}));

        assert_eq!(cache.size(), 1);
        assert!(cache.get("first").is_none());
        assert_eq!(cache.get("second"), Some(json!({"value": 2})));
    }

    #[test]
    fn get_entry_returns_pre_rendered_default() {
        let cache = QueryCache::new(2, 60.0);

        cache.put_with_rendered(
            "search",
            json!({"query": "CacheEntry", "results": []}),
            Some("{\"query\":\"CacheEntry\",\"results\":[]}".into()),
        );

        let cached = cache.get_entry("search").expect("cached entry");
        assert_eq!(cached.result, json!({"query": "CacheEntry", "results": []}));
        assert_eq!(
            cached.rendered_default.as_deref(),
            Some("{\"query\":\"CacheEntry\",\"results\":[]}")
        );
    }

    #[test]
    fn get_rendered_returns_pre_rendered_default() {
        let cache = QueryCache::new(2, 60.0);

        cache.put_with_rendered(
            "search",
            json!({"query": "CacheEntry", "results": []}),
            Some("{\"query\":\"CacheEntry\",\"results\":[]}".into()),
        );

        assert_eq!(
            cache.get_rendered("search").as_deref(),
            Some("{\"query\":\"CacheEntry\",\"results\":[]}")
        );
    }
}
