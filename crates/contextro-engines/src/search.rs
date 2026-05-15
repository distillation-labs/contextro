//! Search execution engine: classify, retrieve, fuse, filter, compact.

use std::collections::{HashMap, HashSet};

use contextro_core::models::SearchResult;

use crate::bm25::Bm25Engine;
use crate::cache::QueryCache;
use crate::fusion::ReciprocalRankFusion;
use crate::graph::CodeGraph;

/// Classify query intent for routing.
pub fn classify_query(query: &str) -> QueryType {
    let q = query.trim();
    if q.is_empty() {
        return QueryType::Hybrid;
    }
    let words: Vec<&str> = q.split_whitespace().collect();
    // Single identifier-like token → symbol lookup
    if words.len() == 1 && (q.contains('_') || q.chars().any(|c| c.is_uppercase())) {
        return QueryType::Symbol;
    }
    // Multi-word natural language
    let identifier_like_words = words
        .iter()
        .filter(|w| w.contains('_') || w.chars().any(|c| c.is_uppercase()))
        .count();
    if words.len() >= 3 && identifier_like_words < words.len() {
        return QueryType::Natural;
    }
    QueryType::Hybrid
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryType {
    Symbol,
    Natural,
    Hybrid,
}

/// Options for search execution.
pub struct SearchOptions {
    pub query: String,
    pub limit: usize,
    pub language: Option<String>,
    pub mode: String,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            query: String::new(),
            limit: 10,
            language: None,
            mode: "hybrid".into(),
        }
    }
}

/// Execute a full search pipeline.
pub fn execute_search(
    options: &SearchOptions,
    bm25: &Bm25Engine,
    graph: &CodeGraph,
    cache: &QueryCache,
    fusion: &ReciprocalRankFusion,
) -> SearchResponse {
    // Check cache first (fast path)
    if let Some(cached) = cache.get(&options.query) {
        if let Ok(resp) = serde_json::from_value(cached) {
            return resp;
        }
    }

    let limit = options.limit.clamp(1, 100);
    let route = classify_query(&options.query);

    // Collect ranked lists from engines
    let mut ranked_lists: HashMap<String, Vec<SearchResult>> = HashMap::with_capacity(3);

    // BM25 retrieval
    if matches!(options.mode.as_str(), "hybrid" | "bm25") {
        let bm25_results = bm25.search(&options.query, limit * 2);
        if !bm25_results.is_empty() {
            ranked_lists.insert("bm25".into(), bm25_results);
        }
    }

    // Graph relevance retrieval (skip for natural language queries)
    if options.mode == "hybrid" && route != QueryType::Natural {
        let graph_results = graph_relevance_search(graph, &options.query, limit);
        if !graph_results.is_empty() {
            ranked_lists.insert("graph".into(), graph_results);
        }
    }

    // Fuse or pass through
    let mut results = if ranked_lists.len() > 1 {
        fusion.fuse(&ranked_lists)
    } else {
        ranked_lists.into_values().next().unwrap_or_default()
    };

    // Relevance threshold filtering
    if let Some(top_score) = results.first().map(|r| r.score) {
        let threshold = top_score * 0.40;
        results.retain(|r| r.score >= threshold);
    }

    // Same-file diversity penalty
    apply_diversity_penalty(&mut results);

    // Graph consensus: boost results that are graph-neighbors of other results
    apply_graph_consensus(&mut results, graph);

    results.truncate(limit);

    let confidence = calculate_confidence(&results);
    let total = results.len();

    let response = SearchResponse {
        query: options.query.clone(),
        results,
        confidence,
        total,
    };

    // Cache the response
    if let Ok(val) = serde_json::to_value(&response) {
        cache.put(&options.query, val);
    }

    response
}

/// Graph-based relevance search using node centrality.
fn graph_relevance_search(graph: &CodeGraph, query: &str, limit: usize) -> Vec<SearchResult> {
    let tokens: Vec<&str> = query.split_whitespace().filter(|t| t.len() >= 2).collect();

    if tokens.is_empty() {
        return vec![];
    }

    let mut candidates = Vec::new();
    for token in &tokens {
        let matches = graph.find_nodes_by_name(token, false);
        for node in matches {
            let (in_deg, out_deg) = graph.get_node_degree(&node.id);
            let score = (in_deg * 2 + out_deg) as f64;
            candidates.push((node, score));
        }
    }

    if candidates.is_empty() {
        return vec![];
    }

    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    candidates.dedup_by(|a, b| a.0.id == b.0.id);

    let max_score = candidates.first().map(|(_, s)| *s).unwrap_or(1.0).max(1.0);

    candidates
        .into_iter()
        .take(limit)
        .map(|(node, score)| SearchResult {
            id: node.id.clone(),
            filepath: node.location.file_path.clone(),
            symbol_name: node.name.clone(),
            symbol_type: node.node_type.to_string(),
            language: node.language.clone(),
            line_start: node.location.start_line,
            line_end: node.location.end_line,
            score: score / max_score,
            code: String::new(),
            signature: String::new(),
            match_sources: vec!["graph".into()],
        })
        .collect()
}

fn apply_diversity_penalty(results: &mut [SearchResult]) {
    let mut file_counts: HashMap<String, usize> = HashMap::new();
    for r in results.iter_mut() {
        let count = file_counts.entry(r.filepath.clone()).or_insert(0);
        *count += 1;
        if *count > 2 {
            r.score *= 0.7; // Penalize 3rd+ result from same file
        }
    }
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Boost results that are graph-neighbors of other results (consensus signal).
fn apply_graph_consensus(results: &mut [SearchResult], graph: &CodeGraph) {
    if results.len() < 2 {
        return;
    }
    let id_set: HashSet<String> = results.iter().map(|r| r.id.clone()).collect();
    let boosts: Vec<f64> = results
        .iter()
        .map(|r| {
            let mut connections = 0usize;
            for caller in graph.get_callers(&r.id) {
                if id_set.contains(&caller.id) {
                    connections += 1;
                }
            }
            for callee in graph.get_callees(&r.id) {
                if id_set.contains(&callee.id) {
                    connections += 1;
                }
            }
            connections as f64 * 0.1
        })
        .collect();
    for (r, b) in results.iter_mut().zip(boosts) {
        r.score += b;
    }
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn calculate_confidence(results: &[SearchResult]) -> String {
    if results.is_empty() {
        return "low".into();
    }
    let top = results[0].score;
    let second = results.get(1).map(|r| r.score).unwrap_or(0.0);
    let gap = top - second;

    if gap > 0.3 && top > 0.7 {
        "high".into()
    } else if top > 0.4 {
        "medium".into()
    } else {
        "low".into()
    }
}

/// Search response structure.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub confidence: String,
    pub total: usize,
}

#[cfg(test)]
mod tests {
    use super::{classify_query, execute_search, QueryType, SearchOptions};
    use crate::bm25::Bm25Engine;
    use crate::cache::QueryCache;
    use crate::fusion::ReciprocalRankFusion;
    use crate::graph::CodeGraph;
    use crate::vector::VectorIndex;
    use contextro_core::models::CodeChunk;

    #[test]
    fn test_classify_query_treats_three_word_descriptions_as_natural() {
        assert_eq!(classify_query("repo_add auto indexes"), QueryType::Natural);
        assert_eq!(
            classify_query("semantic search ranking noise"),
            QueryType::Natural
        );
    }

    #[test]
    fn test_classify_query_keeps_symbol_like_queries_out_of_natural() {
        assert_eq!(classify_query("BrowserSession"), QueryType::Symbol);
        assert_eq!(classify_query("repo_add"), QueryType::Symbol);
    }

    fn make_chunk(id: &str, text: &str, name: &str, filepath: &str) -> CodeChunk {
        CodeChunk {
            id: id.into(),
            text: text.into(),
            filepath: filepath.into(),
            symbol_name: name.into(),
            symbol_type: "function".into(),
            language: "rust".into(),
            line_start: 1,
            line_end: 10,
            signature: format!("pub fn {name}()"),
            parent: String::new(),
            docstring: String::new(),
            vector: vec![],
        }
    }

    #[test]
    fn test_execute_search_surfaces_query_cache_for_conceptual_cache_queries() {
        let bm25 = Bm25Engine::new_in_memory();
        bm25.index_chunks(&[
            make_chunk(
                "cache",
                "Query cache stores search results with TTL eviction and cache invalidation.",
                "QueryCache",
                "crates/contextro-engines/src/cache.rs",
            ),
            make_chunk(
                "search",
                "Search ranking routes requests to hybrid retrieval and reranking.",
                "handle_search",
                "crates/contextro-tools/src/search.rs",
            ),
        ]);

        let response = execute_search(
            &SearchOptions {
                query: "how does caching work".into(),
                limit: 10,
                language: None,
                mode: "bm25".into(),
            },
            &bm25,
            &CodeGraph::new(),
            &QueryCache::new(16, 60.0),
            &ReciprocalRankFusion::default(),
        );

        assert!(!response.results.is_empty());
        assert_eq!(response.results[0].symbol_name, "QueryCache");
        assert_eq!(
            response.results[0].filepath,
            "crates/contextro-engines/src/cache.rs"
        );
    }

    #[test]
    fn test_execute_search_handles_widened_bm25_limit_for_natural_language_queries() {
        let bm25 = Bm25Engine::new_in_memory();
        bm25.index_chunks(&[
            make_chunk(
                "cache",
                "Query cache stores search responses with TTL eviction and cache invalidation.",
                "QueryCache",
                "crates/contextro-engines/src/cache.rs",
            ),
            make_chunk(
                "other",
                "Graph ranking boosts callers and callees for symbol queries.",
                "apply_graph_consensus",
                "crates/contextro-engines/src/search.rs",
            ),
        ]);

        let response = execute_search(
            &SearchOptions {
                query: "how does caching work".into(),
                limit: 40,
                language: None,
                mode: "bm25".into(),
            },
            &bm25,
            &CodeGraph::new(),
            &QueryCache::new(16, 60.0),
            &ReciprocalRankFusion::default(),
        );

        assert!(!response.results.is_empty());
        assert_eq!(response.results[0].symbol_name, "QueryCache");
    }

    #[test]
    fn test_execute_search_surfaces_query_cache_for_ttl_eviction_query_at_limit_ten() {
        let bm25 = Bm25Engine::new_in_memory();
        bm25.index_chunks(&[
            make_chunk(
                "cache",
                "Query cache stores search responses with TTL eviction and cache invalidation.",
                "QueryCache",
                "crates/contextro-engines/src/cache.rs",
            ),
            make_chunk(
                "search",
                "Search routing decides whether to use vector or BM25 retrieval.",
                "handle_search",
                "crates/contextro-tools/src/search.rs",
            ),
        ]);

        let response = execute_search(
            &SearchOptions {
                query: "how does the query cache work, TTL eviction".into(),
                limit: 10,
                language: None,
                mode: "bm25".into(),
            },
            &bm25,
            &CodeGraph::new(),
            &QueryCache::new(16, 60.0),
            &ReciprocalRankFusion::default(),
        );

        assert!(!response.results.is_empty());
        assert_eq!(response.results[0].symbol_name, "QueryCache");
        assert_eq!(
            response.results[0].filepath,
            "crates/contextro-engines/src/cache.rs"
        );
    }

    #[test]
    fn test_execute_search_keeps_chunk_text_for_downstream_reranking() {
        let bm25 = Bm25Engine::new_in_memory();
        bm25.index_chunks(&[make_chunk(
            "cache",
            "Query cache stores search results with TTL eviction and cache invalidation.",
            "QueryCache",
            "crates/contextro-engines/src/cache.rs",
        )]);

        let response = execute_search(
            &SearchOptions {
                query: "query cache ttl eviction".into(),
                limit: 5,
                language: None,
                mode: "bm25".into(),
            },
            &bm25,
            &CodeGraph::new(),
            &QueryCache::new(16, 60.0),
            &ReciprocalRankFusion::default(),
        );

        assert_eq!(response.results.len(), 1);
        assert!(response.results[0]
            .code
            .to_ascii_lowercase()
            .contains("ttl eviction"));
        assert!(VectorIndex::new().is_empty());
    }
}
