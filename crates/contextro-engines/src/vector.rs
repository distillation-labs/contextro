//! In-memory vector index for semantic similarity search.

use contextro_core::models::SearchResult;
use parking_lot::RwLock;

struct VectorEntry {
    vector: Vec<f32>,
    result: SearchResult,
}

/// Thread-safe in-memory vector index with cosine similarity search.
pub struct VectorIndex {
    entries: RwLock<Vec<VectorEntry>>,
}

impl VectorIndex {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(Vec::new()),
        }
    }

    pub fn clear(&self) {
        self.entries.write().clear();
    }

    pub fn insert(&self, vector: Vec<f32>, result: SearchResult) {
        self.entries.write().push(VectorEntry { vector, result });
    }

    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }

    /// Cosine similarity search. Returns up to `limit` results ranked by score.
    pub fn search(&self, query_vec: &[f32], limit: usize) -> Vec<SearchResult> {
        if query_vec.is_empty() {
            return vec![];
        }
        let entries = self.entries.read();
        if entries.is_empty() {
            return vec![];
        }

        let query_norm = l2_norm(query_vec);
        if query_norm == 0.0 {
            return vec![];
        }

        let mut scored: Vec<(f64, usize)> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let dot: f64 = e
                    .vector
                    .iter()
                    .zip(query_vec.iter())
                    .map(|(a, b)| (*a as f64) * (*b as f64))
                    .sum();
                let entry_norm = l2_norm(&e.vector);
                let score = if entry_norm > 0.0 {
                    dot / (query_norm * entry_norm)
                } else {
                    0.0
                };
                (score, i)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        scored
            .iter()
            .take(limit)
            .map(|(score, i)| {
                let mut result = entries[*i].result.clone();
                result.score = *score;
                result.match_sources = vec!["vector".into()];
                result
            })
            .collect()
    }
}

impl Default for VectorIndex {
    fn default() -> Self {
        Self::new()
    }
}

fn l2_norm(v: &[f32]) -> f64 {
    v.iter().map(|x| (*x as f64) * (*x as f64)).sum::<f64>().sqrt()
}
