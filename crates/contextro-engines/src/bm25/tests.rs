use super::*;

fn make_chunk(id: &str, text: &str, name: &str) -> CodeChunk {
    CodeChunk {
        id: id.into(),
        text: text.into(),
        filepath: "src/main.py".into(),
        symbol_name: name.into(),
        symbol_type: "function".into(),
        language: "python".into(),
        line_start: 1,
        line_end: 10,
        signature: format!("def {}():", name),
        parent: String::new(),
        docstring: String::new(),
        vector: vec![],
    }
}

#[test]
fn test_bm25_index_and_search() {
    let engine = Bm25Engine::new_in_memory();
    let chunks = vec![
        make_chunk(
            "c1",
            "authenticate user with JWT token verification",
            "authenticate",
        ),
        make_chunk("c2", "connect to database and run migrations", "connect_db"),
        make_chunk(
            "c3",
            "parse configuration from environment variables",
            "parse_config",
        ),
    ];

    engine.index_chunks(&chunks);
    assert_eq!(engine.count(), 3);

    let results = engine.search("authentication JWT", 10);
    assert!(!results.is_empty());
    assert_eq!(results[0].symbol_name, "authenticate");
}

#[test]
fn test_bm25_delete_and_clear() {
    let engine = Bm25Engine::new_in_memory();
    engine.index_chunks(&[make_chunk("c1", "hello world", "hello")]);
    assert_eq!(engine.count(), 1);

    engine.clear();
    assert_eq!(engine.count(), 0);
}

#[test]
fn test_bm25_search_recovers_cache_from_caching_query() {
    let engine = Bm25Engine::new_in_memory();
    let mut cache_chunk = make_chunk(
        "cache",
        "Query cache stores search responses with TTL eviction and invalidation support.",
        "QueryCache",
    );
    cache_chunk.filepath = "crates/contextro-engines/src/cache.rs".into();
    cache_chunk.signature = "pub struct QueryCache".into();
    let other_chunk = make_chunk(
        "search",
        "Search routing decides whether to use vector or BM25 retrieval.",
        "handle_search",
    );

    engine.index_chunks(&[cache_chunk, other_chunk]);

    let results = engine.search("how does caching work", 5);

    assert!(!results.is_empty());
    assert_eq!(results[0].symbol_name, "QueryCache");
    assert!(results[0]
        .code
        .to_ascii_lowercase()
        .contains("ttl eviction"));
}

#[test]
fn test_bm25_search_uses_concept_tokens_for_query_cache_eviction() {
    let engine = Bm25Engine::new_in_memory();
    let mut cache_chunk = make_chunk(
        "cache",
        "Query cache stores results with TTL eviction and invalidates expired entries.",
        "QueryCache",
    );
    cache_chunk.filepath = "crates/contextro-engines/src/cache.rs".into();
    cache_chunk.signature = "pub struct QueryCache".into();

    let mut fusion_chunk = make_chunk(
        "fusion",
        "Reciprocal rank fusion combines BM25 and vector results.",
        "ReciprocalRankFusion",
    );
    fusion_chunk.filepath = "crates/contextro-engines/src/fusion.rs".into();

    engine.index_chunks(&[cache_chunk, fusion_chunk]);

    let results = engine.search("how does the query cache work TTL eviction", 5);

    assert!(!results.is_empty());
    assert_eq!(results[0].symbol_name, "QueryCache");
}

#[test]
fn test_bm25_search_handles_widened_limits_without_panicking() {
    let engine = Bm25Engine::new_in_memory();
    let mut cache_chunk = make_chunk(
        "cache",
        "Query cache stores search results with TTL eviction and cache invalidation.",
        "QueryCache",
    );
    cache_chunk.filepath = "crates/contextro-engines/src/cache.rs".into();
    cache_chunk.signature = "pub struct QueryCache".into();

    engine.index_chunks(&[cache_chunk]);

    let results = engine.search("how does caching work", 80);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].symbol_name, "QueryCache");
    assert_eq!(results[0].filepath, "crates/contextro-engines/src/cache.rs");
}

#[test]
fn test_bm25_skips_supplemental_variants_when_primary_query_is_well_grounded() {
    let query_terms = collect_query_terms("how does query cache handle ttl eviction");
    let primary_results = vec![SearchResult {
        id: "cache".into(),
        filepath: "crates/contextro-engines/src/cache.rs".into(),
        symbol_name: "QueryCache".into(),
        symbol_type: "struct".into(),
        language: "rust".into(),
        line_start: 1,
        line_end: 10,
        score: 1.0,
        code: "Query cache handle keeps TTL eviction behavior stable.".into(),
        signature: "pub struct QueryCache".into(),
        match_sources: vec!["bm25".into()],
    }];

    assert!(!should_run_supplemental_variants(
        &query_terms,
        &primary_results,
        5,
    ));
}

#[test]
fn test_plain_query_fast_path_detects_query_syntax() {
    assert!(is_plain_bm25_query("query cache eviction"));
    assert!(!is_plain_bm25_query("symbol_name:QueryCache"));
    assert!(!is_plain_bm25_query("query AND cache"));
    assert!(!is_plain_bm25_query("\"query cache\""));
}
