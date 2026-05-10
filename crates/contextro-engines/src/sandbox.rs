//! Output sandbox for large tool results.

use std::collections::VecDeque;
use std::time::Instant;

use parking_lot::Mutex;
use sha2::{Digest, Sha256};

struct SandboxEntry {
    content: String,
    timestamp: Instant,
}

/// Stores large outputs and provides retrieval by reference.
pub struct OutputSandbox {
    inner: Mutex<SandboxInner>,
    max_entries: usize,
    ttl_secs: f64,
}

struct SandboxInner {
    entries: VecDeque<(String, SandboxEntry)>,
}

impl OutputSandbox {
    pub fn new(max_entries: usize, ttl_secs: f64) -> Self {
        Self {
            inner: Mutex::new(SandboxInner { entries: VecDeque::new() }),
            max_entries,
            ttl_secs,
        }
    }

    /// Store content and return a reference ID.
    pub fn store(&self, content: &str) -> String {
        let ref_id = Self::generate_id(content);
        let mut inner = self.inner.lock();

        // Prune expired
        let now = Instant::now();
        inner.entries.retain(|(_, e)| now.duration_since(e.timestamp).as_secs_f64() < self.ttl_secs);

        // Evict if at capacity
        while inner.entries.len() >= self.max_entries {
            inner.entries.pop_front();
        }

        inner.entries.push_back((ref_id.clone(), SandboxEntry {
            content: content.to_string(),
            timestamp: Instant::now(),
        }));

        ref_id
    }

    /// Retrieve stored content by reference ID.
    pub fn retrieve(&self, ref_id: &str) -> Option<String> {
        let inner = self.inner.lock();
        inner.entries.iter()
            .find(|(id, _)| id == ref_id)
            .map(|(_, entry)| entry.content.clone())
    }

    fn generate_id(content: &str) -> String {
        let hash = Sha256::digest(content.as_bytes());
        format!("sx_{}", hex::encode(&hash[..4]))
    }
}

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}
