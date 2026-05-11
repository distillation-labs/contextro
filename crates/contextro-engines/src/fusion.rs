//! Reciprocal Rank Fusion and entropy-adaptive weighting.

use std::collections::HashMap;

use contextro_core::models::SearchResult;

/// Fuse multiple ranked lists using Reciprocal Rank Fusion.
pub struct ReciprocalRankFusion {
    pub weights: HashMap<String, f64>,
    pub k: usize,
}

impl Default for ReciprocalRankFusion {
    fn default() -> Self {
        let mut weights = HashMap::new();
        weights.insert("vector".into(), 0.5);
        weights.insert("bm25".into(), 0.3);
        weights.insert("graph".into(), 0.2);
        Self { weights, k: 60 }
    }
}

impl ReciprocalRankFusion {
    pub fn new(weights: HashMap<String, f64>) -> Self {
        Self { weights, k: 60 }
    }

    /// Fuse ranked lists with entropy-adaptive weighting.
    pub fn fuse(&self, ranked_lists: &HashMap<String, Vec<SearchResult>>) -> Vec<SearchResult> {
        let effective_weights = self.adaptive_weights(ranked_lists);

        // Pre-allocate with estimated capacity
        let total_results: usize = ranked_lists.values().map(|v| v.len()).sum();
        let mut scores: HashMap<String, f64> = HashMap::with_capacity(total_results);
        let mut metadata: HashMap<String, SearchResult> = HashMap::with_capacity(total_results);
        let mut sources: HashMap<String, Vec<String>> = HashMap::with_capacity(total_results);

        for (engine, results) in ranked_lists {
            let weight = effective_weights.get(engine).copied().unwrap_or(0.0);
            if weight <= 0.0 {
                continue;
            }

            for (rank, result) in results.iter().enumerate() {
                let rrf = weight / (self.k as f64 + rank as f64 + 1.0);
                *scores.entry(result.id.clone()).or_default() += rrf;
                metadata.entry(result.id.clone()).or_insert_with(|| result.clone());
                sources.entry(result.id.clone()).or_default().push(engine.clone());
            }
        }

        let mut fused: Vec<SearchResult> = scores
            .into_iter()
            .filter_map(|(id, score)| {
                let mut result = metadata.remove(&id)?;
                result.score = score;
                result.match_sources = sources.remove(&id).unwrap_or_default();
                Some(result)
            })
            .collect();

        fused.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Normalize scores to [0, 1]
        if let Some(max_score) = fused.first().map(|r| r.score) {
            if max_score > 0.0 {
                for r in &mut fused {
                    r.score /= max_score;
                }
            }
        }

        fused
    }

    fn adaptive_weights(&self, ranked_lists: &HashMap<String, Vec<SearchResult>>) -> HashMap<String, f64> {
        // Detect degenerate retrievers (all scores equal)
        let mut degenerate = Vec::new();
        for (engine, results) in ranked_lists {
            if results.len() >= 2 {
                let scores: Vec<f64> = results.iter().map(|r| r.score).collect();
                let range = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                    - scores.iter().cloned().fold(f64::INFINITY, f64::min);
                if range < 1e-6 {
                    degenerate.push(engine.clone());
                }
            }
        }

        if !degenerate.is_empty() {
            let mut adjusted: HashMap<String, f64> = self.weights
                .iter()
                .map(|(e, w)| (e.clone(), if degenerate.contains(e) { 0.0 } else { *w }))
                .collect();
            let total: f64 = adjusted.values().sum();
            if total > 0.0 {
                for w in adjusted.values_mut() {
                    *w /= total;
                }
            }
            return adjusted;
        }

        self.weights.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(id: &str, score: f64) -> SearchResult {
        SearchResult {
            id: id.into(),
            filepath: format!("{}.py", id),
            symbol_name: id.into(),
            symbol_type: "function".into(),
            language: "python".into(),
            line_start: 1,
            line_end: 10,
            score,
            code: String::new(),
            signature: String::new(),
            match_sources: vec![],
        }
    }

    #[test]
    fn test_rrf_fusion() {
        let rrf = ReciprocalRankFusion::default();
        let mut lists = HashMap::new();
        lists.insert("vector".into(), vec![make_result("a", 0.9), make_result("b", 0.7)]);
        lists.insert("bm25".into(), vec![make_result("b", 0.8), make_result("c", 0.6)]);

        let fused = rrf.fuse(&lists);
        assert!(!fused.is_empty());
        // "b" appears in both lists, should rank high
        assert_eq!(fused[0].id, "b");
    }
}
