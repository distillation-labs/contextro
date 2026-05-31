use super::*;

#[test]
fn test_fuse_results_preserves_score_spread() {
    let fused = fuse_results(
        "search ranking noise",
        vec![make_result("lexical_top", 1.0, &["bm25"])],
        vec![make_result("vector_top", 0.96, &["vector"])],
        10,
    );

    assert_eq!(fused.len(), 2);
    assert!(fused[0].score < 1.0);
    assert!(fused[1].score < fused[0].score);
}

#[test]
fn test_fuse_results_rewards_cross_engine_agreement() {
    let fused = fuse_results(
        "search ranking noise",
        vec![
            make_result("shared", 0.92, &["bm25", "graph"]),
            make_result("lexical_only", 1.0, &["bm25"]),
        ],
        vec![make_result("shared", 0.88, &["vector"])],
        10,
    );

    assert_eq!(fused[0].id, "shared");
    assert!(fused[0].score > fused[1].score);
}

#[test]
fn test_fusion_weights_favor_vector_for_natural_language_queries() {
    assert_eq!(
        fusion_weights_for_query("semantic search ranking noise"),
        (0.55, 0.45)
    );
    assert_eq!(fusion_weights_for_query("BrowserSession"), (0.70, 0.30));
}

#[test]
fn test_hybrid_candidate_limit_expands_for_natural_language_queries() {
    assert_eq!(hybrid_candidate_limit("knowledge search milestones", 5), 30);
    assert_eq!(hybrid_candidate_limit("observability config", 5), 30);
    assert_eq!(hybrid_candidate_limit("how does caching work", 5), 30);
    assert_eq!(hybrid_candidate_limit("BrowserSession", 5), 10);
}

#[test]
fn test_rerank_result_limit_expands_for_natural_language_queries() {
    assert_eq!(rerank_result_limit("semantic search ranking noise", 5), 20);
    assert_eq!(rerank_result_limit("observability config", 5), 20);
    assert_eq!(rerank_result_limit("how does caching work", 5), 20);
    assert_eq!(rerank_result_limit("BrowserSession", 5), 5);
}

#[test]
fn test_hybrid_vector_signal_skips_symbol_like_queries() {
    assert!(!should_include_vector_signal_in_hybrid("BrowserSession"));
    assert!(!should_include_vector_signal_in_hybrid("get_settings"));
    assert!(should_include_vector_signal_in_hybrid(
        "how does caching work"
    ));
    assert!(should_include_vector_signal_in_hybrid("caching"));
}

#[test]
fn test_handle_search_surfaces_query_cache_for_caching_queries() {
    let bm25 = Bm25Engine::new_in_memory();
    bm25.index_chunks(&[
            make_chunk(
                "cache",
                "Query cache stores cached search responses with TTL eviction and invalidation support.",
                "QueryCache",
                "crates/contextro-engines/src/cache.rs",
            ),
            make_chunk(
                "search",
                "handle_search routes tool requests and formats tool responses.",
                "handle_search",
                "crates/contextro-tools/src/search.rs",
            ),
        ]);

    let graph = CodeGraph::new();
    let cache = QueryCache::new(16, 60.0);
    let vector = VectorIndex::new();

    for query in [
        "how does caching work",
        "how does the query cache work, TTL eviction",
    ] {
        let result = handle_search(
            &json!({"query": query, "limit": 10}),
            &bm25,
            &graph,
            &cache,
            &vector,
        );
        let top = &result["results"][0];
        assert_eq!(
            top["name"], "QueryCache",
            "unexpected result for {query}: {result}"
        );
        assert_eq!(
            top["file"], "crates/contextro-engines/src/cache.rs",
            "unexpected result for {query}: {result}"
        );
    }
}

#[test]
fn test_handle_search_returns_cached_tool_response() {
    let bm25 = Bm25Engine::new_in_memory();
    let graph = CodeGraph::new();
    let cache = QueryCache::new(16, 60.0);
    let vector = VectorIndex::new();
    let args = json!({
        "query": "cached symbol",
        "limit": 5,
        "mode": "hybrid",
        "context_files": ["src/main.rs", "src/lib.rs"],
    });
    let expected = json!({
        "query": "cached symbol",
        "confidence": "high",
        "results": [{
            "name": "CachedSymbol",
            "file": "src/lib.rs",
            "line": 1,
            "score": 1.0,
        }],
        "limit": 5,
    });

    cache.put(
        &search_tool_cache_key(
            "cached symbol",
            5,
            "hybrid",
            None,
            &["src/lib.rs".into(), "src/main.rs".into()],
            None,
        ),
        expected.clone(),
    );

    let result = handle_search(&args, &bm25, &graph, &cache, &vector);
    assert_eq!(result, expected);
}

#[test]
fn test_handle_search_with_codebase_returns_relative_file_paths() {
    let bm25 = Bm25Engine::new_in_memory();
    bm25.index_chunks(&[make_chunk(
        "query-cache",
        "cached search responses with ttl eviction",
        "QueryCache",
        "/repo/crates/contextro-engines/src/cache.rs",
    )]);
    let graph = CodeGraph::new();
    let cache = QueryCache::new(16, 60.0);
    let vector = VectorIndex::new();

    let result = handle_search_with_codebase(
        &json!({"query": "QueryCache", "limit": 1, "mode": "bm25"}),
        &bm25,
        &graph,
        &cache,
        &vector,
        Some("/repo"),
    );

    assert_eq!(
        result["results"][0]["file"],
        "crates/contextro-engines/src/cache.rs"
    );
    assert!(
        result["results"][0].get("type").is_none(),
        "unexpected result: {result}"
    );
}

#[test]
fn test_handle_search_infers_repo_root_for_absolute_bm25_paths() {
    let repo = std::env::temp_dir().join("contextro-search-infer-bm25");
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
    bm25.index_chunks(&[make_chunk(
        "query-cache",
        "cached search responses with ttl eviction",
        "QueryCache",
        &cache_file.to_string_lossy(),
    )]);
    let graph = CodeGraph::new();
    let cache = QueryCache::new(16, 60.0);
    let vector = VectorIndex::new();

    let result = handle_search(
        &json!({"query": "QueryCache", "limit": 1, "mode": "bm25"}),
        &bm25,
        &graph,
        &cache,
        &vector,
    );

    assert_eq!(
        result["results"][0]["file"],
        "crates/contextro-engines/src/cache.rs"
    );

    let _ = std::fs::remove_dir_all(repo);
}

#[test]
fn test_handle_search_recovers_query_cache_from_explanatory_cache_runtime_pattern() {
    let bm25 = Bm25Engine::new_in_memory();
    bm25.index_chunks(&[
            make_chunk(
                "cache-tests-1",
                "test handle search surfaces query cache for caching queries and cache behavior",
                "test_handle_search_surfaces_query_cache_for_caching_queries",
                "crates/contextro-tools/src/search.rs",
            ),
            make_chunk(
                "cache-tests-2",
                "test bm25 search recovers cache from caching query",
                "test_bm25_search_recovers_cache_from_caching_query",
                "crates/contextro-engines/src/bm25.rs",
            ),
            make_chunk(
                "hf-cache-path",
                "find huggingface cache path on disk",
                "find_hf_cache_path",
                "crates/contextro-indexing/src/embedding.rs",
            ),
            make_chunk(
                "read-cache",
                "reads update cache from disk for release checks",
                "read_cache",
                "crates/contextro-server/src/update_check.rs",
            ),
            make_chunk(
                "write-cache",
                "writes update cache to disk for release checks",
                "write_cache",
                "crates/contextro-server/src/update_check.rs",
            ),
            make_chunk(
                "evict-test",
                "put evicts without deadlocking cache entry replacement",
                "put_evicts_without_deadlocking",
                "crates/contextro-engines/src/cache.rs",
            ),
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
        &json!({"query": "how does caching work", "limit": 10, "mode": "hybrid"}),
        &bm25,
        &graph,
        &cache,
        &vector,
    );

    let results = result["results"].as_array().expect("results array");
    assert_eq!(
        results[0]["name"], "QueryCache",
        "unexpected result: {result}"
    );
    assert_eq!(results[0]["file"], "crates/contextro-engines/src/cache.rs");
    assert!(results.iter().any(|entry| {
        entry["file"] == "crates/contextro-engines/src/cache.rs"
            && entry["name"] == "QueryCache::get"
    }));
}

#[test]
fn test_drop_low_confidence_noise_removes_nonsense_hits() {
    let filtered = drop_low_confidence_noise(
        "xyznonexistent999",
        "bm25",
        vec![make_named_result(
            "noise",
            "test_knowledge_add_rejects_nonexistent_path_like_value",
            "crates/contextro-tools/src/memory.rs",
            0.0674,
            &["bm25"],
        )],
    );

    assert!(filtered.is_empty());
}

#[test]
fn test_drop_low_confidence_noise_prunes_vector_tail_noise() {
    let filtered = drop_low_confidence_noise(
        "session archive persistence across restart",
        "vector",
        vec![
            make_named_result(
                "top-hit",
                "handle_retrieve",
                "crates/contextro-tools/src/session.rs",
                0.42,
                &["vector"],
            ),
            make_named_result(
                "tail-hit",
                "random_helper",
                "crates/contextro-tools/src/search.rs",
                0.21,
                &["vector"],
            ),
            make_named_result(
                "noise-hit",
                "test_search_fixture",
                "crates/contextro-tools/src/search.rs",
                0.12,
                &["vector"],
            ),
        ],
    );

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].symbol_name, "handle_retrieve");
}

#[test]
fn test_drop_low_confidence_noise_vector_rejects_digit_bearing_nonsense_without_literal_match() {
    let filtered = drop_low_confidence_noise(
        "xyznonexistent999",
        "vector",
        vec![
            make_named_result(
                "noise-1",
                "test_knowledge_add_rejects_nonexistent_path_like_value",
                "crates/contextro-tools/src/memory.rs",
                0.46,
                &["vector"],
            ),
            make_named_result(
                "noise-2",
                "test_repo_add_reports_non_git_directory",
                "crates/contextro-tools/src/git_tools.rs",
                0.41,
                &["vector"],
            ),
        ],
    );

    assert!(filtered.is_empty());
}

#[test]
fn test_drop_low_confidence_noise_vector_keeps_digit_query_with_literal_grounding() {
    let filtered = drop_low_confidence_noise(
        "repo_add_v2",
        "vector",
        vec![make_named_result(
            "real-hit",
            "handle_repo_add_v2",
            "crates/contextro-tools/src/git_tools.rs",
            0.43,
            &["vector"],
        )],
    );

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].symbol_name, "handle_repo_add_v2");
}

#[test]
fn test_handle_search_vector_mode_respects_language_filter() {
    let filtered = filter_results_by_language(
        vec![
            make_named_result_with_language(
                "rust-hit",
                "RustSearch",
                "crates/contextro-tools/src/search.rs",
                "rust",
                0.42,
                &["vector"],
            ),
            make_named_result_with_language(
                "python-hit",
                "PythonSearch",
                "crates/contextro-tools/src/search.py",
                "python",
                0.39,
                &["vector"],
            ),
        ],
        Some("Python"),
    );

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].symbol_name, "PythonSearch");
}
