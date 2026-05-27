use super::*;

#[test]
fn test_handle_search_query_cache_exact_match_reports_high_confidence() {
    let bm25 = Bm25Engine::new_in_memory();
    bm25.index_chunks(&[
        make_chunk(
            "query-cache",
            "QueryCache stores cached search responses with TTL eviction and invalidation support.",
            "QueryCache",
            "crates/contextro-engines/src/cache.rs",
        ),
        make_chunk(
            "query-cache-get",
            "QueryCache::get returns cached search responses when cache entries have not expired.",
            "QueryCache::get",
            "crates/contextro-engines/src/cache.rs",
        ),
        make_chunk(
            "handle-search",
            "handle_search routes search tool requests and formats responses.",
            "handle_search",
            "crates/contextro-tools/src/search.rs",
        ),
    ]);

    let graph = CodeGraph::new();
    let cache = QueryCache::new(16, 60.0);
    let vector = VectorIndex::new();

    let result = handle_search(
        &json!({"query": "QueryCache", "limit": 10, "mode": "hybrid"}),
        &bm25,
        &graph,
        &cache,
        &vector,
    );

    assert_eq!(
        result["results"][0]["name"], "QueryCache",
        "unexpected result: {result}"
    );
    assert_eq!(result["confidence"], "high", "unexpected result: {result}");
}

#[test]
fn test_confidence_label_promotes_exact_symbol_like_top_hit_without_large_gap() {
    let confidence = confidence_label(
        "QueryCache",
        &[
            make_named_result(
                "query-cache",
                "QueryCache",
                "crates/contextro-engines/src/cache.rs",
                0.62,
                &["bm25"],
            ),
            make_named_result(
                "query-cache-get",
                "QueryCache::get",
                "crates/contextro-engines/src/cache.rs",
                0.58,
                &["bm25"],
            ),
        ],
    );

    assert_eq!(confidence, "high");
}

#[test]
fn test_confidence_label_keeps_natural_language_cache_query_at_medium_without_large_gap() {
    let confidence = confidence_label(
        "how does caching work",
        &[
            make_named_result(
                "query-cache",
                "QueryCache",
                "crates/contextro-engines/src/cache.rs",
                0.62,
                &["bm25"],
            ),
            make_named_result(
                "query-cache-get",
                "QueryCache::get",
                "crates/contextro-engines/src/cache.rs",
                0.58,
                &["bm25"],
            ),
        ],
    );

    assert_eq!(confidence, "medium");
}

#[test]
fn test_handle_search_keeps_rich_payload_for_exact_symbol_matches() {
    let bm25 = Bm25Engine::new_in_memory();
    bm25.index_chunks(&[
        make_chunk(
            "query-cache",
            "QueryCache stores cached search responses with TTL eviction and invalidation support.",
            "QueryCache",
            "crates/contextro-engines/src/cache.rs",
        ),
        make_chunk(
            "query-cache-get",
            "QueryCache::get returns cached search responses when cache entries have not expired.",
            "QueryCache::get",
            "crates/contextro-engines/src/cache.rs",
        ),
    ]);

    let graph = CodeGraph::new();
    let cache = QueryCache::new(16, 60.0);
    let vector = VectorIndex::new();

    let result = handle_search(
        &json!({"query": "QueryCache", "limit": 10, "mode": "bm25"}),
        &bm25,
        &graph,
        &cache,
        &vector,
    );

    assert_eq!(result["confidence"], "high", "unexpected result: {result}");
    assert_eq!(result["results"][0]["name"], "QueryCache");
    assert_eq!(
        result["results"][0]["file"],
        "crates/contextro-engines/src/cache.rs"
    );
    assert_eq!(result["results"][0]["type"], "function");
    assert!(
        result["results"][0].get("score").is_some(),
        "unexpected result: {result}"
    );
    assert_eq!(result["limit"], 10);
    assert!(
        result.get("truncated").is_none(),
        "unexpected result: {result}"
    );
}

#[test]
fn test_handle_search_exact_symbol_uses_graph_without_bm25_hits() {
    let bm25 = Bm25Engine::new_in_memory();
    let graph = CodeGraph::new();
    graph.add_node(make_graph_node(
        "query-cache-node",
        "QueryCache",
        "crates/contextro-engines/src/cache.rs",
        "rust",
    ));
    let cache = QueryCache::new(16, 60.0);
    let vector = VectorIndex::new();

    let result = handle_search(
        &json!({"query": "QueryCache", "limit": 10, "mode": "hybrid"}),
        &bm25,
        &graph,
        &cache,
        &vector,
    );

    assert_eq!(result["results"][0]["name"], "QueryCache");
    assert_eq!(
        result["results"][0]["file"],
        "crates/contextro-engines/src/cache.rs"
    );
    assert!(
        result["results"][0].get("type").is_none(),
        "unexpected result: {result}"
    );
    assert_eq!(result["confidence"], "high", "unexpected result: {result}");
}

#[test]
fn test_handle_search_infers_repo_root_for_absolute_graph_paths() {
    let repo = std::env::temp_dir().join("contextro-search-infer-graph");
    let _ = std::fs::remove_dir_all(&repo);
    std::fs::create_dir_all(repo.join(".git")).unwrap();

    let cache_file = repo.join("crates/contextro-engines/src/cache.rs");
    std::fs::create_dir_all(cache_file.parent().unwrap()).unwrap();
    std::fs::write(
        &cache_file,
        "pub struct QueryCache;
",
    )
    .unwrap();

    let bm25 = Bm25Engine::new_in_memory();
    let graph = CodeGraph::new();
    graph.add_node(make_graph_node(
        "query-cache-node",
        "QueryCache",
        &cache_file.to_string_lossy(),
        "rust",
    ));
    let cache = QueryCache::new(16, 60.0);
    let vector = VectorIndex::new();

    let result = handle_search(
        &json!({"query": "QueryCache", "limit": 10, "mode": "hybrid"}),
        &bm25,
        &graph,
        &cache,
        &vector,
    );

    assert_eq!(result["results"][0]["name"], "QueryCache");
    assert_eq!(
        result["results"][0]["file"],
        "crates/contextro-engines/src/cache.rs"
    );

    let _ = std::fs::remove_dir_all(repo);
}

#[test]
fn test_handle_search_exact_symbol_fast_path_respects_language_filter() {
    let bm25 = Bm25Engine::new_in_memory();
    let graph = CodeGraph::new();
    graph.add_node(make_graph_node(
        "query-cache-rust",
        "QueryCache",
        "crates/contextro-engines/src/cache.rs",
        "rust",
    ));
    graph.add_node(make_graph_node(
        "query-cache-ts",
        "QueryCache",
        "packages/cache/query-cache.ts",
        "typescript",
    ));
    let cache = QueryCache::new(16, 60.0);
    let vector = VectorIndex::new();

    let result = handle_search(
        &json!({"query": "QueryCache", "limit": 10, "mode": "bm25", "language": "typescript"}),
        &bm25,
        &graph,
        &cache,
        &vector,
    );

    let results = result["results"].as_array().expect("results array");
    assert_eq!(results.len(), 1, "unexpected result: {result}");
    assert_eq!(results[0]["file"], "packages/cache/query-cache.ts");
}

#[test]
fn test_handle_search_keeps_rich_payload_for_non_exact_single_term_queries() {
    let bm25 = Bm25Engine::new_in_memory();
    bm25.index_chunks(&[
            make_chunk(
                "query-cache",
                "Query cache stores cached search responses with TTL eviction and invalidation support.",
                "QueryCache",
                "crates/contextro-engines/src/cache.rs",
            ),
            make_chunk(
                "query-cache-get",
                "Returns cached search responses when query cache entries have not expired.",
                "QueryCache::get",
                "crates/contextro-engines/src/cache.rs",
            ),
        ]);

    let graph = CodeGraph::new();
    let cache = QueryCache::new(16, 60.0);
    let vector = VectorIndex::new();

    let result = handle_search(
        &json!({"query": "cache", "limit": 10, "mode": "bm25"}),
        &bm25,
        &graph,
        &cache,
        &vector,
    );

    let entries = result["results"].as_array().expect("results array");
    assert!(entries.iter().any(|entry| entry["name"] == "QueryCache"));
    assert!(
        entries.iter().all(|entry| entry.get("type").is_some()),
        "unexpected result: {result}"
    );
    assert!(
        entries.iter().all(|entry| entry.get("score").is_some()),
        "unexpected result: {result}"
    );
    assert_eq!(result["limit"], 10);
    assert!(
        result.get("truncated").is_none(),
        "unexpected result: {result}"
    );
}
