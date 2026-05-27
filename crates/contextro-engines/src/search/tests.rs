use super::{
    cache_key, classify_query, execute_search, should_include_graph_signal, QueryType,
    SearchOptions,
};
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

#[test]
fn test_graph_signal_only_runs_for_hybrid_queries() {
    assert!(should_include_graph_signal("hybrid", QueryType::Hybrid));
    assert!(!should_include_graph_signal("hybrid", QueryType::Symbol));
    assert!(!should_include_graph_signal("hybrid", QueryType::Natural));
    assert!(!should_include_graph_signal("bm25", QueryType::Hybrid));
}

fn make_chunk(id: &str, text: &str, name: &str, filepath: &str) -> CodeChunk {
    make_chunk_with_language(id, text, name, filepath, "rust")
}

fn make_chunk_with_language(
    id: &str,
    text: &str,
    name: &str,
    filepath: &str,
    language: &str,
) -> CodeChunk {
    CodeChunk {
        id: id.into(),
        text: text.into(),
        filepath: filepath.into(),
        symbol_name: name.into(),
        symbol_type: "function".into(),
        language: language.into(),
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

#[test]
fn test_execute_search_cache_key_includes_limit() {
    let cache = QueryCache::new(16, 60.0);

    let first_key = cache_key(&SearchOptions {
        query: "query cache".into(),
        limit: 1,
        language: None,
        mode: "bm25".into(),
    });
    let second_key = cache_key(&SearchOptions {
        query: "query cache".into(),
        limit: 2,
        language: None,
        mode: "bm25".into(),
    });

    cache.put(&first_key, serde_json::json!({"limit": 1}));
    cache.put(&second_key, serde_json::json!({"limit": 2}));

    assert_eq!(cache.get(&first_key), Some(serde_json::json!({"limit": 1})));
    assert_eq!(
        cache.get(&second_key),
        Some(serde_json::json!({"limit": 2}))
    );
}

#[test]
fn test_execute_search_respects_language_filter() {
    let bm25 = Bm25Engine::new_in_memory();
    bm25.index_chunks(&[
        make_chunk_with_language(
            "rust-cache",
            "Query cache stores cached search responses with TTL eviction and invalidation.",
            "RustCache",
            "crates/contextro-engines/src/cache.rs",
            "rust",
        ),
        make_chunk_with_language(
            "python-cache",
            "Query cache stores cached search responses with TTL eviction and invalidation.",
            "PythonCache",
            "crates/contextro-engines/src/cache.py",
            "python",
        ),
    ]);

    let response = execute_search(
        &SearchOptions {
            query: "query cache".into(),
            limit: 10,
            language: Some("Python".into()),
            mode: "bm25".into(),
        },
        &bm25,
        &CodeGraph::new(),
        &QueryCache::new(16, 60.0),
        &ReciprocalRankFusion::default(),
    );

    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].symbol_name, "PythonCache");
    assert_eq!(response.results[0].language, "python");
}
