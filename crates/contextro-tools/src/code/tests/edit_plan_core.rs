use super::*;

#[test]
fn test_edit_plan_only_returns_exact_related_test_matches() {
    let graph = CodeGraph::new();
    graph.add_node(UniversalNode {
        id: "truncate-chars".into(),
        name: "truncate_chars".into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: "/tmp/contextro/crates/contextro-tools/src/code.rs".into(),
            start_line: 14,
            end_line: 21,
            start_column: 0,
            end_column: 0,
            language: "rust".into(),
        },
        language: "rust".into(),
        ..Default::default()
    });

    let result = edit_plan(
        &json!({"goal":"rename truncate_chars to truncate_text","symbol_name":"truncate_chars"}),
        &graph,
        Some("/tmp/contextro"),
    );

    assert_eq!(result["related_tests"].as_array().unwrap().len(), 0);
}

#[test]
fn test_edit_plan_infers_scope_from_goal_only() {
    let graph = CodeGraph::new();
    let file = "/tmp/contextro/crates/contextro-tools/src/search.rs";
    graph.add_node(test_node(
        "handle-search",
        "handle_search",
        file,
        17,
        "pub fn handle_search() { rerank_natural_language_results(); drop_low_confidence_noise(); }",
    ));
    graph.add_node(test_node(
        "rerank",
        "rerank_natural_language_results",
        file,
        21,
        "fn rerank_natural_language_results() {}",
    ));
    graph.add_node(test_node(
        "drop-noise",
        "drop_low_confidence_noise",
        file,
        130,
        "fn drop_low_confidence_noise() {}",
    ));
    add_call(&graph, "handle-search", "rerank");
    add_call(&graph, "handle-search", "drop-noise");

    let result = edit_plan(
        &json!({"goal":"refactor handle_search to extract reranking into a separate function"}),
        &graph,
        Some("/tmp/contextro"),
    );

    let affected = result["affected_symbols"].as_array().unwrap();
    let names: Vec<&str> = affected
        .iter()
        .filter_map(|symbol| symbol["name"].as_str())
        .collect();

    assert_eq!(result["confidence"], "high");
    assert_eq!(
        result["target_files"][0],
        "crates/contextro-tools/src/search.rs"
    );
    assert!(names.contains(&"handle_search"));
    assert!(names.contains(&"rerank_natural_language_results"));
    assert!(names.contains(&"drop_low_confidence_noise"));
    assert!(result["next_steps"]
        .as_array()
        .unwrap()
        .iter()
        .all(|step| step.as_str() != Some("Review the diff preview before applying")));
}
#[test]
fn test_edit_plan_reports_low_confidence_for_empty_scope() {
    let graph = CodeGraph::new();
    let result = edit_plan(
        &json!({"goal":"refactor impossible_missing_symbol to extract helper"}),
        &graph,
        Some("/tmp/contextro"),
    );

    assert_eq!(result["confidence"], "low");
    assert_eq!(result["affected_symbols"].as_array().unwrap().len(), 0);
    assert_eq!(result["target_files"].as_array().unwrap().len(), 0);
    assert!(result["next_steps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|step| step.as_str() == Some("Resolve the target symbol or file before editing")));
}
#[test]
fn test_edit_plan_prioritizes_cross_subsystem_anchors_before_generic_goal_tokens() {
    let graph = CodeGraph::new();
    let server_file = "/tmp/contextro/crates/contextro-server/src/main.rs";
    let state_file = "/tmp/contextro/crates/contextro-server/src/state.rs";
    let cache_file = "/tmp/contextro/crates/contextro-engines/src/cache.rs";
    let unrelated_file = "/tmp/contextro/crates/contextro-tools/src/code.rs";

    graph.add_node(test_node(
        "dispatch",
        "dispatch",
        server_file,
        50,
        "fn dispatch(&self, name: &str, args: Value) { self.state.query_cache.get(name); }",
    ));
    graph.add_node(test_node(
        "app-state",
        "AppState",
        state_file,
        27,
        "pub struct AppState { pub query_cache: Arc<QueryCache>, pub graph: Arc<CodeGraph> }",
    ));
    graph.add_node(test_node(
        "query-cache",
        "QueryCache",
        cache_file,
        14,
        "pub struct QueryCache { entries: DashMap<String, CacheEntry> }",
    ));
    graph.add_node(test_node(
        "generic-add",
        "add_edit_plan_symbol",
        unrelated_file,
        2166,
        "fn add_edit_plan_symbol() {}",
    ));
    graph.add_node(test_node(
        "generic-result",
        "accumulate_result",
        unrelated_file,
        1400,
        "fn accumulate_result() { result.push(value); }",
    ));

    let result = edit_plan(
        &json!({"goal":"add per-tool result caching to the dispatch function using QueryCache"}),
        &graph,
        Some("/tmp/contextro"),
    );

    let affected = result["affected_symbols"].as_array().unwrap();
    let names: Vec<&str> = affected
        .iter()
        .filter_map(|symbol| symbol["name"].as_str())
        .collect();
    let primary_names: Vec<&str> = affected
        .iter()
        .filter(|symbol| symbol["role"].as_str() == Some("primary"))
        .filter_map(|symbol| symbol["name"].as_str())
        .collect();
    let target_files: Vec<&str> = result["target_files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|value| value.as_str())
        .collect();

    assert_eq!(result["confidence"], "high");
    assert!(primary_names.contains(&"dispatch"));
    assert!(primary_names.contains(&"QueryCache"));
    assert!(names.contains(&"AppState"));
    assert!(!primary_names.contains(&"add_edit_plan_symbol"));
    assert!(!primary_names.contains(&"accumulate_result"));
    assert!(target_files.contains(&"crates/contextro-server/src/main.rs"));
    assert!(target_files.contains(&"crates/contextro-server/src/state.rs"));
    assert!(target_files.contains(&"crates/contextro-engines/src/cache.rs"));
}

#[test]
fn test_edit_plan_bridge_expansion_adds_same_file_and_conceptual_bridge_symbols() {
    let graph = CodeGraph::new();
    let server_file = "/tmp/contextro/crates/contextro-server/src/main.rs";
    let state_file = "/tmp/contextro/crates/contextro-server/src/state.rs";
    let cache_file = "/tmp/contextro/crates/contextro-engines/src/cache.rs";

    graph.add_node(test_node(
        "dispatch",
        "dispatch",
        server_file,
        50,
        "fn dispatch(&self, name: &str, args: Value) { let s = &self.state; }",
    ));
    graph.add_node(test_node(
        "contextro-server",
        "ContextroServer",
        server_file,
        21,
        "pub struct ContextroServer { state: Arc<AppState> }",
    ));
    graph.add_node(test_node(
        "app-state",
        "AppState",
        state_file,
        27,
        "pub struct AppState { pub query_cache: Arc<QueryCache> }",
    ));
    graph.add_node(test_node(
        "query-cache-field",
        "query_cache",
        state_file,
        32,
        "pub query_cache: Arc<QueryCache>",
    ));
    graph.add_node(test_node(
        "query-cache",
        "QueryCache",
        cache_file,
        14,
        "pub struct QueryCache { entries: DashMap<String, CacheEntry> }",
    ));

    let result = edit_plan(
        &json!({"goal":"add per-tool result caching to the dispatch function using QueryCache"}),
        &graph,
        Some("/tmp/contextro"),
    );

    let affected = result["affected_symbols"].as_array().unwrap();
    let names: Vec<&str> = affected
        .iter()
        .filter_map(|symbol| symbol["name"].as_str())
        .collect();

    assert!(names.contains(&"dispatch"));
    assert!(names.contains(&"ContextroServer"));
    assert!(names.contains(&"AppState"));
    assert!(names.contains(&"query_cache") || names.contains(&"QueryCache"));
}

#[test]
fn test_edit_plan_prefers_state_bridge_symbols_and_filters_test_like_distractors() {
    let graph = CodeGraph::new();
    let server_file = "/tmp/contextro/crates/contextro-server/src/main.rs";
    let state_file = "/tmp/contextro/crates/contextro-server/src/state.rs";
    let cache_file = "/tmp/contextro/crates/contextro-engines/src/cache.rs";
    let tool_file = "/tmp/contextro/crates/contextro-tools/src/code.rs";
    let test_file = "/tmp/contextro/crates/contextro-server/tests/dispatch.rs";

    graph.add_node(test_node(
        "dispatch",
        "dispatch",
        server_file,
        50,
        "fn dispatch(&self, name: &str, args: Value) { self.state.query_cache.get(name); }",
    ));
    graph.add_node(test_node(
        "server",
        "ContextroServer",
        server_file,
        21,
        "pub struct ContextroServer { state: Arc<AppState> }",
    ));
    graph.add_node(test_node(
        "app-state",
        "AppState",
        state_file,
        27,
        "pub struct AppState { pub query_cache: Arc<QueryCache>, pub graph: Arc<CodeGraph> }",
    ));
    graph.add_node(test_node(
        "query-cache-field",
        "query_cache",
        state_file,
        32,
        "pub query_cache: Arc<QueryCache>",
    ));
    graph.add_node(test_node(
        "query-cache",
        "QueryCache",
        cache_file,
        14,
        "pub struct QueryCache { entries: DashMap<String, CacheEntry> }",
    ));
    graph.add_node(test_node(
        "helper-add",
        "add_edit_plan_symbol",
        tool_file,
        2544,
        "fn add_edit_plan_symbol() { affected_symbols.push(value); }",
    ));
    graph.add_node(test_node(
        "helper-accumulate",
        "accumulate_result",
        tool_file,
        1404,
        "fn accumulate_result() { result.push(value); }",
    ));
    graph.add_node(test_node(
        "dispatch-test",
        "test_dispatch_query_cache",
        test_file,
        10,
        "fn test_dispatch_query_cache() { assert!(true); }",
    ));

    add_call(&graph, "server", "dispatch");
    add_call(&graph, "dispatch", "app-state");
    add_call(&graph, "app-state", "query-cache");
    add_call(&graph, "app-state", "query-cache-field");
    add_call(&graph, "dispatch-test", "dispatch");
    add_call(&graph, "helper-accumulate", "dispatch");

    let result = edit_plan(
        &json!({"goal":"add per-tool result caching to the dispatch function using QueryCache"}),
        &graph,
        Some("/tmp/contextro"),
    );

    let affected = result["affected_symbols"].as_array().unwrap();
    let names: Vec<&str> = affected
        .iter()
        .filter_map(|symbol| symbol["name"].as_str())
        .collect();

    assert!(names.contains(&"dispatch"), "unexpected names: {:?}", names);
    assert!(
        names.contains(&"QueryCache"),
        "unexpected names: {:?}",
        names
    );
    assert!(names.contains(&"AppState"), "unexpected names: {:?}", names);
    assert!(
        !names.contains(&"test_dispatch_query_cache"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        !names.contains(&"add_edit_plan_symbol"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        !names.contains(&"accumulate_result"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        affected.len() <= 5,
        "too many affected symbols returned: {:?}",
        names
    );
}
