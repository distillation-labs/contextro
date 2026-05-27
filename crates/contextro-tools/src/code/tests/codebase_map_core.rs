use super::*;
#[test]
fn test_search_codebase_map_surfaces_conceptual_cluster() {
    let graph = CodeGraph::new();
    let file = "/tmp/contextro/crates/contextro-tools/src/search.rs";
    graph.add_node(test_node(
        "handle-search",
        "handle_search",
        file,
        17,
        "pub fn handle_search() { rerank_natural_language_results(); }",
    ));
    graph.add_node(test_node(
        "rerank",
        "rerank_natural_language_results",
        file,
        21,
        "fn rerank_natural_language_results() { score search ranking candidates }",
    ));
    graph.add_node(test_node(
        "drop-noise",
        "drop_low_confidence_noise",
        file,
        130,
        "fn drop_low_confidence_noise() { filter weak ranking results }",
    ));
    graph.add_node(test_node(
        "lookup-query",
        "is_symbol_lookup_query",
        file,
        386,
        "fn is_symbol_lookup_query() { detect symbol search queries }",
    ));
    graph.add_node(test_node(
        "vector-limit",
        "vector_candidate_limit",
        file,
        552,
        "fn vector_candidate_limit() -> usize { ranking search candidates.len() }",
    ));
    graph.add_node(test_node(
        "symbol-match",
        "result_matches_symbol_query",
        file,
        398,
        "fn result_matches_symbol_query() { score whether search results match the symbol query }",
    ));
    graph.add_node(test_node(
        "resolve-targets",
        "resolve_refactor_targets",
        "/tmp/contextro/crates/contextro-tools/src/refactor.rs",
        80,
        "fn resolve_refactor_targets() { resolve targets for edit plans }",
    ));
    graph.add_node(test_node(
        "rank-degree",
        "rank_nodes_by_degree",
        "/tmp/contextro/crates/contextro-engines/src/graph.rs",
        44,
        "fn rank_nodes_by_degree() { rank graph nodes by degree }",
    ));
    add_call(&graph, "handle-search", "rerank");
    add_call(&graph, "handle-search", "drop-noise");
    add_call(&graph, "handle-search", "lookup-query");
    add_call(&graph, "drop-noise", "lookup-query");
    add_call(&graph, "drop-noise", "symbol-match");
    add_call(&graph, "rerank", "vector-limit");
    add_call(&graph, "rank-degree", "rerank");

    let result = search_codebase_map(
        &json!({"query":"how does search ranking work"}),
        &graph,
        Some("/tmp/contextro"),
    );

    assert_eq!(result["total_files"], 1);
    assert!(result["total_symbols"].as_u64().unwrap_or(0) >= 3);
    let names: Vec<&str> = result["files"][0]["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|symbol| symbol["name"].as_str())
        .collect();
    assert_eq!(
        result["files"][0]["file"],
        "crates/contextro-tools/src/search.rs"
    );
    assert!(names.contains(&"handle_search"));
    assert!(names.contains(&"rerank_natural_language_results"));
    assert!(names.contains(&"drop_low_confidence_noise"));
    assert!(names.contains(&"is_symbol_lookup_query"));
    assert!(names.contains(&"result_matches_symbol_query"));
    assert!(names.contains(&"vector_candidate_limit"));
    assert!(
        !names.contains(&"resolve_refactor_targets"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        !names.contains(&"rank_nodes_by_degree"),
        "unexpected names: {:?}",
        names
    );
}

#[test]
fn test_search_codebase_map_prefers_product_surface_over_engine_internals() {
    let graph = CodeGraph::new();
    let tool_file = "/tmp/contextro/crates/contextro-tools/src/search.rs";
    let engine_file = "/tmp/contextro/crates/contextro-engines/src/search.rs";

    graph.add_node(test_node(
        "tool-handle-search",
        "handle_search",
        tool_file,
        17,
        "pub fn handle_search() { rerank_natural_language_results(); drop_low_confidence_noise(); }",
    ));
    graph.add_node(test_node(
        "tool-rerank",
        "rerank_natural_language_results",
        tool_file,
        216,
        "fn rerank_natural_language_results() { improve search ranking for product responses }",
    ));
    graph.add_node(test_node(
        "tool-noise",
        "drop_low_confidence_noise",
        tool_file,
        130,
        "fn drop_low_confidence_noise() { prune weak ranking results from tool output }",
    ));
    graph.add_node(test_node(
        "engine-execute",
        "execute_search",
        engine_file,
        61,
        "pub fn execute_search() { fuse(); }",
    ));
    graph.add_node(test_node(
        "engine-fuse",
        "fuse",
        "/tmp/contextro/crates/contextro-engines/src/fusion.rs",
        29,
        "fn fuse() { adaptive_weights(); }",
    ));
    graph.add_node(test_node(
        "engine-adaptive",
        "adaptive_weights",
        "/tmp/contextro/crates/contextro-engines/src/fusion.rs",
        85,
        "fn adaptive_weights() { rank vector and bm25 search inputs }",
    ));
    add_call(&graph, "tool-handle-search", "tool-rerank");
    add_call(&graph, "tool-handle-search", "tool-noise");
    add_call(&graph, "engine-execute", "engine-fuse");
    add_call(&graph, "engine-fuse", "engine-adaptive");

    let result = search_codebase_map(
        &json!({"query":"how does search ranking work"}),
        &graph,
        Some("/tmp/contextro"),
    );

    assert_eq!(
        result["files"][0]["file"],
        "crates/contextro-tools/src/search.rs"
    );
    let names: Vec<&str> = result["files"][0]["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|symbol| symbol["name"].as_str())
        .collect();
    assert!(
        names.contains(&"handle_search"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        names.contains(&"rerank_natural_language_results"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        names.contains(&"drop_low_confidence_noise"),
        "unexpected names: {:?}",
        names
    );
}
#[test]
fn test_search_codebase_map_prefers_dominant_same_file_subsystem_closure() {
    let graph = CodeGraph::new();
    let search_file = "/tmp/contextro/crates/contextro-tools/src/search.rs";

    graph.add_node(test_node(
        "handle-search",
        "handle_search",
        search_file,
        17,
        "pub fn handle_search() { rerank_natural_language_results(); drop_low_confidence_noise(); }",
    ));
    graph.add_node(test_node(
        "rerank",
        "rerank_natural_language_results",
        search_file,
        99,
        "fn rerank_natural_language_results() { search ranking response output rerank results }",
    ));
    graph.add_node(test_node(
        "drop-noise",
        "drop_low_confidence_noise",
        search_file,
        130,
        "fn drop_low_confidence_noise() { search ranking noise output response filtering }",
    ));
    graph.add_node(test_node(
        "lookup-query",
        "is_symbol_lookup_query",
        search_file,
        386,
        "fn is_symbol_lookup_query() -> bool { detect search symbol query before ranking }",
    ));
    graph.add_node(test_node(
        "symbol-match",
        "result_matches_symbol_query",
        search_file,
        398,
        "fn result_matches_symbol_query() { keep search ranking results that match the symbol query }",
    ));
    graph.add_node(test_node(
        "vector-limit",
        "vector_candidate_limit",
        search_file,
        552,
        "fn vector_candidate_limit() -> usize { choose search ranking candidate limits for result reranking }",
    ));
    graph.add_node(test_node(
        "query-targets",
        "query_targets_product_surface",
        search_file,
        304,
        "fn query_targets_product_surface(query: &str) -> bool { detect whether search asks about product surface output }",
    ));
    graph.add_node(test_node(
        "fuse-results",
        "fuse_results",
        search_file,
        479,
        "fn fuse_results(query: &str) { accumulate_result(query); }",
    ));
    graph.add_node(test_node(
        "accumulate-result",
        "accumulate_result",
        search_file,
        565,
        "fn accumulate_result() { update fused score maps }",
    ));

    graph.add_node(test_node(
        "resolve-targets",
        "resolve_refactor_targets",
        "/tmp/contextro/crates/contextro-tools/src/refactor.rs",
        80,
        "fn resolve_refactor_targets() { resolve targets for edit plan changes }",
    ));
    graph.add_node(test_node(
        "rank-degree",
        "rank_nodes_by_degree",
        "/tmp/contextro/crates/contextro-engines/src/graph.rs",
        44,
        "fn rank_nodes_by_degree() { graph ranking nodes by degree for connectivity }",
    ));
    graph.add_node(test_node(
        "query-targets",
        "query_targets_product_surface",
        "/tmp/contextro/crates/contextro-tools/src/code.rs",
        80,
        "fn query_targets_product_surface() -> bool { detect whether the query asks about tool output }",
    ));

    add_call(&graph, "handle-search", "rerank");
    add_call(&graph, "handle-search", "drop-noise");
    add_call(&graph, "handle-search", "lookup-query");
    add_call(&graph, "drop-noise", "lookup-query");
    add_call(&graph, "drop-noise", "symbol-match");
    add_call(&graph, "rerank", "vector-limit");
    add_call(&graph, "rerank", "fuse-results");
    add_call(&graph, "fuse-results", "accumulate-result");
    add_call(&graph, "query-targets", "handle-search");
    add_call(&graph, "rank-degree", "rerank");
    add_call(&graph, "query-targets", "handle-search");

    let result = search_codebase_map(
        &json!({"query":"how does search ranking work"}),
        &graph,
        Some("/tmp/contextro"),
    );

    assert_eq!(result["total_files"], 1, "unexpected result: {result}");
    assert_eq!(
        result["files"][0]["file"],
        "crates/contextro-tools/src/search.rs"
    );

    let names: Vec<&str> = result["files"][0]["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|symbol| symbol["name"].as_str())
        .collect();

    assert!(
        names.contains(&"handle_search"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        names.contains(&"rerank_natural_language_results"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        names.contains(&"drop_low_confidence_noise"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        names.contains(&"is_symbol_lookup_query"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        names.contains(&"result_matches_symbol_query"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        names.contains(&"vector_candidate_limit"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        !names.contains(&"resolve_refactor_targets"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        !names.contains(&"rank_nodes_by_degree"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        !names.contains(&"query_targets_product_surface"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        !names.contains(&"fuse_results"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        !names.contains(&"accumulate_result"),
        "unexpected names: {:?}",
        names
    );
}

#[test]
fn test_search_codebase_map_prefers_cache_module_for_explanatory_cache_queries() {
    let graph = CodeGraph::new();
    let cache_file = "/tmp/contextro/crates/contextro-engines/src/cache.rs";
    let tool_file = "/tmp/contextro/crates/contextro-tools/src/search.rs";

    graph.add_node(test_node(
        "query-cache",
        "QueryCache",
        cache_file,
        14,
        "pub struct QueryCache { ttl eviction invalidation cached query search responses }",
    ));
    graph.add_node(test_node(
        "cache-get",
        "QueryCache.get",
        cache_file,
        34,
        "fn get(&self, query: &str) -> Option<Value> { ttl expiry returns cached response }",
    ));
    graph.add_node(test_node(
        "cache-put",
        "QueryCache.put",
        cache_file,
        49,
        "fn put(&self, query: &str, result: Value) { cache eviction removes oldest entry at capacity }",
    ));
    graph.add_node(test_node(
        "handle-search",
        "handle_search",
        tool_file,
        17,
        "pub fn handle_search() { execute_search(); rerank_natural_language_results(); }",
    ));
    graph.add_node(test_node(
        "rerank",
        "rerank_natural_language_results",
        tool_file,
        216,
        "fn rerank_natural_language_results() { improve search ranking for product responses }",
    ));

    add_call(&graph, "query-cache", "cache-get");
    add_call(&graph, "query-cache", "cache-put");
    add_call(&graph, "handle-search", "rerank");

    let result = search_codebase_map(
        &json!({"query":"how does the query cache work, TTL eviction"}),
        &graph,
        Some("/tmp/contextro"),
    );

    assert_eq!(result["total_files"], 1, "unexpected result: {result}");
    assert_eq!(
        result["files"][0]["file"],
        "crates/contextro-engines/src/cache.rs"
    );
    let names: Vec<&str> = result["files"][0]["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|symbol| symbol["name"].as_str())
        .collect();
    assert!(
        names.contains(&"QueryCache"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        names.contains(&"QueryCache.get"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        names.contains(&"QueryCache.put"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        !names.contains(&"handle_search"),
        "unexpected names: {:?}",
        names
    );
}

#[test]
fn test_codebase_map_stemming_restores_cache_from_caching_queries() {
    let tokens = tokenize_codebase_map_text("how does caching work");

    assert!(tokens.contains(&"caching".to_string()));
    assert!(tokens.contains(&"cache".to_string()));
}
