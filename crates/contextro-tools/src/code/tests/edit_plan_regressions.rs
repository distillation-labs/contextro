use super::*;
#[test]
fn test_edit_plan_real_runtime_query_stays_focused_on_dispatch_cache_path() {
    let graph = CodeGraph::new();
    let main_file = "/tmp/contextro/crates/contextro-server/src/main.rs";
    let state_file = "/tmp/contextro/crates/contextro-server/src/state.rs";
    let cache_file = "/tmp/contextro/crates/contextro-engines/src/cache.rs";
    let bm25_file = "/tmp/contextro/crates/contextro-engines/src/bm25.rs";
    let chunk_file = "/tmp/contextro/crates/contextro-tools/src/search.rs";
    let analysis_file = "/tmp/contextro/crates/contextro-tools/src/analysis.rs";

    graph.add_node(test_node(
        "dispatch",
        "dispatch",
        main_file,
        50,
        "fn dispatch(&self, name: &str, args: Value) { self.state.query_cache.get(name); self.state.query_cache.insert(name, args); }",
    ));
    graph.add_node(test_node(
        "server",
        "ContextroServer",
        main_file,
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
        "query-cache",
        "QueryCache",
        cache_file,
        14,
        "pub struct QueryCache { entries: DashMap<String, CacheEntry> }",
    ));
    graph.add_node(test_node(
        "dead-code",
        "handle_dead_code",
        analysis_file,
        322,
        "pub fn handle_dead_code() { /* unrelated analysis handler */ }",
    ));
    graph.add_node(test_node(
        "circular",
        "handle_circular_dependencies",
        analysis_file,
        429,
        "pub fn handle_circular_dependencies() { /* unrelated dependency handler */ }",
    ));
    graph.add_node(test_node(
        "search",
        "search",
        main_file,
        110,
        "fn search() { /* dispatches search tools */ }",
    ));
    graph.add_node(test_node(
        "make-chunk",
        "make_chunk",
        chunk_file,
        696,
        "fn make_chunk(id: &str, text: &str) -> CodeChunk { /* chunk builder */ }",
    ));
    graph.add_node(test_node(
        "bm25-cache-test",
        "test_bm25_search_recovers_cache_from_caching_query",
        bm25_file,
        389,
        "fn test_bm25_search_recovers_cache_from_caching_query() { assert!(cache_hit); }",
    ));

    add_call(&graph, "server", "dispatch");
    add_call(&graph, "dispatch", "app-state");
    add_call(&graph, "app-state", "query-cache");
    add_call(&graph, "dispatch", "search");
    add_call(&graph, "dispatch", "dead-code");
    add_call(&graph, "dispatch", "circular");
    add_call(&graph, "dispatch", "make-chunk");

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

    assert!(
        primary_names.contains(&"dispatch"),
        "unexpected primaries: {:?}",
        primary_names
    );
    assert!(
        primary_names.contains(&"QueryCache"),
        "unexpected primaries: {:?}",
        primary_names
    );
    assert!(names.contains(&"AppState"), "unexpected names: {:?}", names);
    assert!(
        !names.contains(&"test_bm25_search_recovers_cache_from_caching_query"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        !names.contains(&"handle_circular_dependencies"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        !names.contains(&"handle_dead_code"),
        "unexpected names: {:?}",
        names
    );
    assert!(!names.contains(&"search"), "unexpected names: {:?}", names);
    assert!(
        !names.contains(&"make_chunk"),
        "unexpected names: {:?}",
        names
    );
    assert!(target_files.contains(&"crates/contextro-server/src/main.rs"));
    assert!(target_files.contains(&"crates/contextro-server/src/state.rs"));
    assert!(target_files.contains(&"crates/contextro-engines/src/cache.rs"));
    assert_eq!(
        target_files.len(),
        3,
        "unexpected target files: {:?}",
        target_files
    );
    assert!(
        affected.len() <= 5,
        "unexpected affected symbols: {:?}",
        names
    );
}

#[test]
fn test_edit_plan_regression_excludes_rc_dispatch_cache_contamination() {
    let graph = CodeGraph::new();
    let code_file = "/tmp/contextro/crates/contextro-tools/src/code.rs";
    let analysis_file = "/tmp/contextro/crates/contextro-tools/src/analysis.rs";

    graph.add_node(test_node(
        "dispatch",
        "dispatch",
        code_file,
        50,
        "fn dispatch(&self, operation: &str, args: Value) { self.state.query_cache.get(operation); self.state.query_cache.insert(operation, CacheEntry::new()); }",
    ));
    graph.add_node(test_node(
        "query-cache",
        "QueryCache",
        code_file,
        14,
        "pub struct QueryCache { entries: DashMap<String, CacheEntry> }",
    ));
    graph.add_node(test_node(
        "cache-entry",
        "CacheEntry",
        code_file,
        19,
        "pub struct CacheEntry { value: Value, hit_rate: usize }",
    ));
    graph.add_node(test_node(
        "server",
        "ContextroServer",
        code_file,
        5,
        "pub struct ContextroServer { state: Arc<AppState> }",
    ));
    graph.add_node(test_node(
        "app-state",
        "AppState",
        code_file,
        9,
        "pub struct AppState { pub query_cache: Arc<QueryCache>, pub graph: Arc<CodeGraph> }",
    ));
    graph.add_node(test_node(
        "handle-index",
        "handle_index",
        code_file,
        80,
        "fn handle_index(state: &AppState) { dispatch(); }",
    ));
    graph.add_node(test_node(
        "handle-focus",
        "handle_focus",
        code_file,
        90,
        "fn handle_focus(state: &AppState) { dispatch(); }",
    ));
    graph.add_node(test_node(
        "handle-status",
        "handle_status",
        code_file,
        100,
        "fn handle_status(state: &AppState) { dispatch(); }",
    ));
    graph.add_node(test_node(
        "clear-scope",
        "clear_active_scope",
        code_file,
        110,
        "fn clear_active_scope(state: &AppState) { state.query_cache.clear(); }",
    ));
    graph.add_node(test_node(
        "hit-rate",
        "hit_rate",
        code_file,
        120,
        "fn hit_rate(entry: &CacheEntry) -> f64 { entry.hit_rate as f64 }",
    ));
    graph.add_node(test_node(
        "analysis-plan",
        "build_analysis_plan",
        analysis_file,
        40,
        "fn build_analysis_plan() { handle_focus(); handle_status(); }",
    ));

    add_call(&graph, "server", "dispatch");
    add_call(&graph, "dispatch", "app-state");
    add_call(&graph, "app-state", "query-cache");
    add_call(&graph, "query-cache", "cache-entry");
    add_call(&graph, "dispatch", "handle-index");
    add_call(&graph, "dispatch", "handle-focus");
    add_call(&graph, "dispatch", "handle-status");
    add_call(&graph, "dispatch", "clear-scope");
    add_call(&graph, "dispatch", "hit-rate");
    add_call(&graph, "analysis-plan", "handle-focus");
    add_call(&graph, "analysis-plan", "handle-status");

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
    let target_files: Vec<&str> = result["target_files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|value| value.as_str())
        .collect();

    assert!(names.contains(&"dispatch"), "unexpected names: {:?}", names);
    assert!(
        names.contains(&"QueryCache"),
        "unexpected names: {:?}",
        names
    );
    assert!(names.contains(&"AppState"), "unexpected names: {:?}", names);
    assert!(
        !names.contains(&"handle_focus"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        !names.contains(&"handle_status"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        !names.contains(&"clear_active_scope"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        !target_files.contains(&"crates/contextro-tools/src/analysis.rs"),
        "unexpected target files: {:?}",
        target_files
    );
    assert_eq!(target_files, vec!["crates/contextro-tools/src/code.rs"]);
}
#[test]
fn test_edit_plan_regression_promotes_app_state_and_excludes_leaky_main_bridges() {
    let graph = CodeGraph::new();
    let main_file = "/tmp/contextro/crates/contextro-server/src/main.rs";
    let state_file = "/tmp/contextro/crates/contextro-server/src/state.rs";
    let cache_file = "/tmp/contextro/crates/contextro-engines/src/cache.rs";

    graph.add_node(test_node(
        "dispatch",
        "dispatch",
        main_file,
        90,
        "fn dispatch(&self, operation: &str, args: Value) { self.state.query_cache.get(operation); call_tool(operation, args); }",
    ));
    graph.add_node(test_node(
        "call-tool",
        "call_tool",
        main_file,
        140,
        "fn call_tool(operation: &str, args: Value) -> Value { dispatch_tool(operation, args) }",
    ));
    graph.add_node(test_node(
        "server",
        "ContextroServer",
        main_file,
        20,
        "pub struct ContextroServer { state: Arc<AppState> }",
    ));
    graph.add_node(test_node(
        "handle-status",
        "handle_status",
        main_file,
        200,
        "fn handle_status(state: &AppState) -> Value { state.query_cache.hit_rate(); json!({}) }",
    ));
    graph.add_node(test_node(
        "clear-scope",
        "clear_active_scope",
        main_file,
        215,
        "fn clear_active_scope(state: &AppState) { state.query_cache.clear(); }",
    ));
    graph.add_node(test_node(
        "app-state",
        "AppState",
        state_file,
        25,
        "pub struct AppState { pub query_cache: Arc<QueryCache>, pub config: Arc<Config> }",
    ));
    graph.add_node(test_node(
        "query-cache",
        "QueryCache",
        cache_file,
        14,
        "pub struct QueryCache { entries: DashMap<String, CacheEntry> }",
    ));
    graph.add_node(test_node(
        "cache-entry",
        "CacheEntry",
        cache_file,
        32,
        "pub struct CacheEntry { value: Value, hit_rate: f64 }",
    ));
    graph.add_node(test_node(
        "hit-rate",
        "hit_rate",
        cache_file,
        60,
        "fn hit_rate(entry: &CacheEntry) -> f64 { entry.hit_rate }",
    ));

    add_call(&graph, "server", "dispatch");
    add_call(&graph, "dispatch", "call-tool");
    add_call(&graph, "dispatch", "app-state");
    add_call(&graph, "app-state", "query-cache");
    add_call(&graph, "query-cache", "cache-entry");
    add_call(&graph, "handle-status", "app-state");
    add_call(&graph, "handle-status", "query-cache");
    add_call(&graph, "clear-scope", "app-state");
    add_call(&graph, "clear-scope", "query-cache");
    add_call(&graph, "hit-rate", "cache-entry");

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
    let target_files: Vec<&str> = result["target_files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|value| value.as_str())
        .collect();
    let app_state_index = names.iter().position(|name| *name == "AppState");
    let handle_status_index = names.iter().position(|name| *name == "handle_status");
    let clear_scope_index = names.iter().position(|name| *name == "clear_active_scope");

    assert!(names.contains(&"dispatch"), "unexpected names: {:?}", names);
    assert!(
        names.contains(&"QueryCache"),
        "unexpected names: {:?}",
        names
    );
    assert!(names.contains(&"AppState"), "unexpected names: {:?}", names);
    assert!(
        names.contains(&"call_tool") || names.contains(&"ContextroServer"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        handle_status_index.is_none(),
        "unexpected names: {:?}",
        names
    );
    assert!(clear_scope_index.is_none(), "unexpected names: {:?}", names);
    assert!(
        target_files.contains(&"crates/contextro-server/src/main.rs"),
        "unexpected target files: {:?}",
        target_files
    );
    assert!(
        target_files.contains(&"crates/contextro-server/src/state.rs"),
        "unexpected target files: {:?}",
        target_files
    );
    assert!(
        target_files.contains(&"crates/contextro-engines/src/cache.rs"),
        "unexpected target files: {:?}",
        target_files
    );
    if let (Some(app_state_index), Some(call_tool_index)) = (
        app_state_index,
        names.iter().position(|name| *name == "call_tool"),
    ) {
        assert!(
            app_state_index < call_tool_index,
            "unexpected ordering: {:?}",
            names
        );
    }
    if let (Some(app_state_index), Some(server_index)) = (
        app_state_index,
        names.iter().position(|name| *name == "ContextroServer"),
    ) {
        assert!(
            app_state_index < server_index,
            "unexpected ordering: {:?}",
            names
        );
    }
}
