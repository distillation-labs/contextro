use super::*;
#[test]
fn test_search_codebase_map_avoids_padding_narrow_commit_search_queries() {
    let graph = CodeGraph::new();
    let commit_file = "/tmp/contextro/crates/contextro-tools/src/git.rs";
    let search_file = "/tmp/contextro/crates/contextro-tools/src/search.rs";
    let code_file = "/tmp/contextro/crates/contextro-tools/src/code.rs";

    graph.add_node(test_node(
        "handle-commit-search",
        "handle_commit_search",
        commit_file,
        12,
        "pub fn handle_commit_search() { search_commit_history(); index_commit_messages(); }",
    ));
    graph.add_node(test_node(
        "search-commit-history",
        "search_commit_history",
        commit_file,
        48,
        "fn search_commit_history() { commit search history ranking query }",
    ));
    graph.add_node(test_node(
        "index-commit-messages",
        "index_commit_messages",
        commit_file,
        88,
        "fn index_commit_messages() { index commit messages for search history }",
    ));

    graph.add_node(test_node(
        "handle-search",
        "handle_search",
        search_file,
        17,
        "pub fn handle_search() { rerank_results(); search query results }",
    ));
    graph.add_node(test_node(
        "rerank-results",
        "rerank_results",
        search_file,
        120,
        "fn rerank_results() { search ranking output results }",
    ));
    graph.add_node(test_node(
        "search-codebase-map",
        "search_codebase_map",
        code_file,
        220,
        "fn search_codebase_map() { search query symbols files }",
    ));

    add_call(&graph, "handle-commit-search", "search-commit-history");
    add_call(&graph, "search-commit-history", "index-commit-messages");
    add_call(&graph, "handle-search", "rerank-results");

    let result = search_codebase_map(
        &json!({"query":"how does commit search work"}),
        &graph,
        Some("/tmp/contextro"),
    );

    assert_eq!(result["total_files"], 1, "unexpected result: {result}");
    assert_eq!(
        result["files"][0]["file"],
        "crates/contextro-tools/src/git.rs"
    );

    let names: Vec<&str> = result["files"][0]["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|symbol| symbol["name"].as_str())
        .collect();
    assert!(
        names.contains(&"handle_commit_search"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        names.contains(&"search_commit_history"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        !names.contains(&"handle_search"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        !names.contains(&"search_codebase_map"),
        "unexpected names: {:?}",
        names
    );
}

#[test]
fn test_search_codebase_map_keeps_broad_architectural_queries_multi_file() {
    let graph = CodeGraph::new();
    let tool_file = "/tmp/contextro/crates/contextro-tools/src/git.rs";
    let git_file = "/tmp/contextro/crates/contextro-git/src/commit_index.rs";

    graph.add_node(test_node(
        "handle-commit-search",
        "handle_commit_search",
        tool_file,
        12,
        "pub fn handle_commit_search() { search_commit_history(); }",
    ));
    graph.add_node(test_node(
        "search-commit-history",
        "search_commit_history",
        tool_file,
        48,
        "fn search_commit_history() { commit search architecture pipeline routes queries to git history }",
    ));
    graph.add_node(test_node(
        "commit-index",
        "CommitIndex",
        git_file,
        20,
        "pub struct CommitIndex { commit search architecture pipeline storage }",
    ));
    graph.add_node(test_node(
        "search-commit-messages",
        "search_commit_messages",
        git_file,
        75,
        "fn search_commit_messages() { commit search architecture pipeline ranking }",
    ));

    add_call(&graph, "handle-commit-search", "search-commit-history");
    add_call(&graph, "search-commit-history", "search-commit-messages");

    let result = search_codebase_map(
        &json!({"query":"commit search architecture pipeline"}),
        &graph,
        Some("/tmp/contextro"),
    );

    assert!(
        result["total_files"].as_u64().unwrap_or(0) >= 2,
        "unexpected result: {result}"
    );
    let files: Vec<&str> = result["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|file| file["file"].as_str())
        .collect();
    assert!(
        files.contains(&"crates/contextro-tools/src/git.rs"),
        "unexpected files: {:?}",
        files
    );
    assert!(
        files.contains(&"crates/contextro-git/src/commit_index.rs"),
        "unexpected files: {:?}",
        files
    );
}
#[test]
fn test_search_codebase_map_prefers_git_tools_for_narrow_commit_queries() {
    let graph = CodeGraph::new();
    let main_file = "/tmp/contextro/crates/contextro-server/src/main.rs";
    let git_file = "/tmp/contextro/crates/contextro-tools/src/git_tools.rs";

    graph.add_node(test_node(
        "dispatch",
        "dispatch",
        main_file,
        10,
        "fn dispatch() { commit_search(); commit_history(); call_tool(); tool_definitions(); }",
    ));
    graph.add_node(test_node(
        "tool-definitions",
        "tool_definitions",
        main_file,
        40,
        "fn tool_definitions() { register commit_search and commit_history handlers }",
    ));
    graph.add_node(test_node(
        "call-tool",
        "call_tool",
        main_file,
        60,
        "fn call_tool() { dispatches git tool handlers }",
    ));
    graph.add_node(test_node(
        "commit-search-route",
        "commit_search",
        main_file,
        72,
        "fn commit_search() { route commit search tool requests }",
    ));
    graph.add_node(test_node(
        "commit-history-route",
        "commit_history",
        main_file,
        86,
        "fn commit_history() { route commit history tool requests }",
    ));

    graph.add_node(test_node(
        "handle-commit-search",
        "handle_commit_search",
        git_file,
        15,
        "pub fn handle_commit_search() { search commit history and rank commit records }",
    ));
    graph.add_node(test_node(
        "handle-commit-history",
        "handle_commit_history",
        git_file,
        36,
        "pub fn handle_commit_history() { load commit records from repo registry }",
    ));
    graph.add_node(test_node(
        "repo-registry",
        "RepoRegistry",
        git_file,
        58,
        "pub struct RepoRegistry { commit search storage and routing helpers }",
    ));
    graph.add_node(test_node(
        "commit-record",
        "CommitRecord",
        git_file,
        84,
        "pub struct CommitRecord { commit message and history payload }",
    ));

    add_call(&graph, "dispatch", "commit-search-route");
    add_call(&graph, "dispatch", "commit-history-route");
    add_call(&graph, "commit-search-route", "handle-commit-search");
    add_call(&graph, "commit-history-route", "handle-commit-history");
    add_call(&graph, "handle-commit-search", "repo-registry");
    add_call(&graph, "handle-commit-search", "commit-record");

    let result = search_codebase_map(
        &json!({"query":"how does commit search work"}),
        &graph,
        Some("/tmp/contextro"),
    );

    let files = result["files"].as_array().unwrap();
    assert!(!files.is_empty(), "unexpected result: {result}");
    assert_eq!(
        files[0]["file"],
        json!("crates/contextro-tools/src/git_tools.rs"),
        "unexpected result: {result}"
    );
}

#[test]
fn test_search_codebase_map_prefers_engine_owner_file_for_bm25_indexing_queries() {
    let graph = CodeGraph::new();
    let main_file = "/tmp/contextro/crates/contextro-server/src/main.rs";
    let bm25_file = "/tmp/contextro/crates/contextro-engines/src/bm25.rs";

    graph.add_node(test_node(
        "dispatch",
        "dispatch",
        main_file,
        10,
        "fn dispatch() { handle_index(); tool_definitions(); }",
    ));
    graph.add_node(test_node(
        "handle-index",
        "handle_index",
        main_file,
        44,
        "fn handle_index() { initialize bm25 index and load search engine state }",
    ));
    graph.add_node(test_node(
        "tool-definitions",
        "tool_definitions",
        main_file,
        80,
        "fn tool_definitions() { register index and search tools that rely on bm25 }",
    ));

    graph.add_node(test_node(
        "bm25-engine",
        "Bm25Engine",
        bm25_file,
        22,
        "pub struct Bm25Engine { bm25 index reader writer schema }",
    ));
    graph.add_node(test_node(
        "index-chunks",
        "index_chunks",
        bm25_file,
        99,
        "pub fn index_chunks() { build bm25 index from code chunks }",
    ));
    graph.add_node(test_node(
        "build-query",
        "build_query",
        bm25_file,
        180,
        "fn build_query() { parse bm25 index query terms with tantivy }",
    ));
    graph.add_node(test_node(
        "plain-query",
        "build_plain_token_query",
        bm25_file,
        227,
        "fn build_plain_token_query() { create bm25 token query for indexing terms }",
    ));

    add_call(&graph, "dispatch", "handle-index");
    add_call(&graph, "handle-index", "index-chunks");
    add_call(&graph, "index-chunks", "build-query");
    add_call(&graph, "build-query", "plain-query");

    let result = search_codebase_map(
        &json!({"query":"how does BM25 indexing work"}),
        &graph,
        Some("/tmp/contextro"),
    );

    assert_eq!(result["total_files"], 1, "unexpected result: {result}");
    assert_eq!(
        result["files"][0]["file"],
        "crates/contextro-engines/src/bm25.rs"
    );
    let names: Vec<&str> = result["files"][0]["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|symbol| symbol["name"].as_str())
        .collect();
    assert!(
        names.contains(&"Bm25Engine"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        names.contains(&"index_chunks"),
        "unexpected names: {:?}",
        names
    );
    assert!(
        !names.contains(&"handle_index"),
        "unexpected names: {:?}",
        names
    );
}
