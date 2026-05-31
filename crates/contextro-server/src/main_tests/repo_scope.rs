use super::*;

#[test]
fn test_repo_remove_restores_previous_active_scope_and_knowledge_scope() {
    let storage_dir = temp_storage_dir("repo-remove-restore");
    let server = test_server(&storage_dir);
    let repo_a = temp_repo_dir("repo-a");
    let repo_b = temp_repo_dir("repo-b");
    write_indexable_repo(&repo_a, "repo_a_symbol");
    write_indexable_repo(&repo_b, "repo_b_symbol");

    let index_a = server.handle_index(&json!({"path": repo_a.to_string_lossy().to_string()}));
    assert_eq!(index_a["status"], "done");
    let repo_a_bm25 = server.state.active_bm25();
    server
        .state
        .knowledge
        .add("repo-a-doc", "alpha scope", None);

    server.dispatch(
        "repo_add",
        json!({"path": repo_b.to_string_lossy().to_string()}),
    );
    assert_eq!(
        server
            .state
            .codebase_path
            .read()
            .clone()
            .map(|path| normalize_repo_dir(&path)),
        Some(normalize_repo_dir(repo_b.to_string_lossy().as_ref()))
    );
    server
        .state
        .knowledge
        .add("repo-b-doc", "bravo scope", None);

    let remove_result =
        server.handle_repo_remove(&json!({"path": repo_b.to_string_lossy().to_string()}));

    assert_eq!(remove_result["removed"], true);
    assert_eq!(remove_result["active_scope_restored"], true);
    assert_eq!(
        server
            .state
            .codebase_path
            .read()
            .clone()
            .map(|path| normalize_repo_dir(&path)),
        Some(normalize_repo_dir(repo_a.to_string_lossy().as_ref()))
    );
    assert_eq!(server.state.knowledge.search("alpha", 5).len(), 1);
    assert!(server.state.knowledge.search("bravo", 5).is_empty());
    assert!(std::sync::Arc::ptr_eq(
        &repo_a_bm25,
        &server.state.active_bm25()
    ));

    let restored_codebase = server.state.codebase_path.read().clone();
    let overview = contextro_tools::analysis::handle_overview(
        &server.state.graph,
        restored_codebase.as_deref(),
        server
            .state
            .chunk_count
            .load(std::sync::atomic::Ordering::Relaxed),
        server.state.vector_index.len(),
    );
    let architecture = contextro_tools::analysis::handle_architecture(
        &json!({}),
        &server.state.graph,
        restored_codebase.as_deref(),
    );
    let bm25 = server.state.active_bm25();
    let repo_a_search = contextro_tools::search::handle_search(
        &json!({"query": "repo_a_symbol"}),
        &bm25,
        &server.state.graph,
        &server.state.query_cache,
        &server.state.vector_index,
    );
    let repo_b_search = contextro_tools::search::handle_search(
        &json!({"query": "repo_b_symbol"}),
        &bm25,
        &server.state.graph,
        &server.state.query_cache,
        &server.state.vector_index,
    );
    let repo_a_results = repo_a_search["results"].as_array().expect("results array");
    let repo_b_results = repo_b_search["results"].as_array().expect("results array");

    assert_eq!(
        overview["codebase_path"],
        normalize_repo_dir(repo_a.to_string_lossy().as_ref())
    );
    assert!(overview["total_symbols"].as_u64().unwrap_or(0) >= 1);
    assert!(architecture["total_nodes"].as_u64().unwrap_or(0) >= 1);
    assert!(repo_a_search.get("total").is_none());
    assert!(repo_a_results
        .iter()
        .any(|result| result["name"] == "repo_a_symbol"));
    assert!(!repo_b_results
        .iter()
        .any(|result| result["name"] == "repo_b_symbol"));

    let _ = std::fs::remove_dir_all(repo_a);
    let _ = std::fs::remove_dir_all(repo_b);
    let _ = std::fs::remove_dir_all(storage_dir);
}

#[test]
fn test_repo_remove_reindexes_previous_scope_when_cached_snapshot_is_stale() {
    let storage_dir = temp_storage_dir("repo-remove-stale-snapshot");
    let server = test_server(&storage_dir);
    let repo_a = temp_repo_dir("repo-stale-a");
    let repo_b = temp_repo_dir("repo-stale-b");
    write_indexable_repo(&repo_a, "repo_a_symbol");
    write_indexable_repo(&repo_b, "repo_b_symbol");

    let index_a = server.handle_index(&json!({"path": repo_a.to_string_lossy().to_string()}));
    assert_eq!(index_a["status"], "done");
    let add_b = server.dispatch(
        "repo_add",
        json!({"path": repo_b.to_string_lossy().to_string()}),
    );
    assert_ne!(add_b.is_error, Some(true));

    write_indexable_repo(&repo_a, "repo_a_updated_symbol");

    let remove_result =
        server.handle_repo_remove(&json!({"path": repo_b.to_string_lossy().to_string()}));
    assert_eq!(remove_result["removed"], true);
    assert_eq!(remove_result["active_scope_restored"], true);

    let search_updated = server.handle_search(&json!({"query": "repo_a_updated_symbol"}));
    let updated_results = search_updated["results"].as_array().expect("results array");
    assert!(updated_results
        .iter()
        .any(|result| result["name"] == "repo_a_updated_symbol"));

    let search_old = server.handle_search(&json!({"query": "repo_a_symbol"}));
    let old_results = search_old["results"].as_array().expect("results array");
    assert!(!old_results
        .iter()
        .any(|result| result["name"] == "repo_a_symbol"));

    let _ = std::fs::remove_dir_all(repo_a);
    let _ = std::fs::remove_dir_all(repo_b);
    let _ = std::fs::remove_dir_all(storage_dir);
}

#[test]
fn test_remember_repo_scope_attaches_graph_snapshot_on_scope_handoff() {
    let storage_dir = temp_storage_dir("repo-scope-graph-handoff");
    let repo_a = temp_repo_dir("repo-handoff-a");
    let repo_b = temp_repo_dir("repo-handoff-b");
    write_indexable_repo(&repo_a, "repo_handoff_a_symbol");
    write_indexable_repo(&repo_b, "repo_handoff_b_symbol");

    let server = test_server(&storage_dir);
    let indexed_a = server.handle_index(&json!({"path": repo_a.to_string_lossy().to_string()}));
    assert_eq!(indexed_a["status"], "done");
    let normalized_a = normalize_repo_dir(repo_a.to_string_lossy().as_ref());
    assert!(server
        .state
        .repo_snapshot(&normalized_a)
        .expect("repo A snapshot")
        .graph
        .is_empty());

    let indexed_b = server.handle_index(&json!({"path": repo_b.to_string_lossy().to_string()}));
    assert_eq!(indexed_b["status"], "done");
    assert!(!server
        .state
        .repo_snapshot(&normalized_a)
        .expect("repo A snapshot after handoff")
        .graph
        .is_empty());

    let _ = std::fs::remove_dir_all(repo_a);
    let _ = std::fs::remove_dir_all(repo_b);
    let _ = std::fs::remove_dir_all(storage_dir);
}

#[test]
fn test_repo_remove_clears_active_scope_when_no_previous_scope_exists() {
    let storage_dir = temp_storage_dir("repo-remove-clear");
    let server = test_server(&storage_dir);
    let repo = temp_repo_dir("repo-clear");
    write_indexable_repo(&repo, "repo_clear_symbol");

    server
        .state
        .repo_registry
        .add(repo.to_string_lossy().as_ref(), None);
    let index_result = server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
    assert_eq!(index_result["status"], "done");
    server
        .state
        .knowledge
        .add("repo-doc", "repo scoped note", None);

    let remove_result =
        server.handle_repo_remove(&json!({"path": repo.to_string_lossy().to_string()}));

    assert_eq!(remove_result["removed"], true);
    assert_eq!(remove_result["active_scope_cleared"], true);
    assert!(!(*server.state.indexed.read()));
    assert_eq!(*server.state.codebase_path.read(), None);
    assert_eq!(server.state.graph.node_count(), 0);
    assert_eq!(
        server
            .state
            .chunk_count
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    assert!(server.state.knowledge.search("repo scoped", 5).is_empty());
    assert!(remove_result["warning"]
        .as_str()
        .unwrap_or("")
        .contains("no previous repo scope"));

    let overview = contextro_tools::analysis::handle_overview(
        &server.state.graph,
        server.state.codebase_path.read().as_deref(),
        server
            .state
            .chunk_count
            .load(std::sync::atomic::Ordering::Relaxed),
        server.state.vector_index.len(),
    );
    let architecture = contextro_tools::analysis::handle_architecture(
        &json!({}),
        &server.state.graph,
        server.state.codebase_path.read().as_deref(),
    );
    let search = contextro_tools::search::handle_search(
        &json!({"query": "repo_clear_symbol"}),
        &server.state.active_bm25(),
        &server.state.graph,
        &server.state.query_cache,
        &server.state.vector_index,
    );

    assert_eq!(overview["codebase_path"], Value::Null);
    assert_eq!(overview["total_symbols"], 0);
    assert_eq!(architecture["total_nodes"], 0);
    assert!(search.get("total").is_none());
    assert_eq!(search["results"].as_array().map_or(0, Vec::len), 0);

    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(storage_dir);
}

#[test]
fn test_handle_index_restores_from_persisted_snapshot_when_repo_is_unchanged() {
    let initial_storage_dir = temp_storage_dir("persisted-snapshot-source");
    let restore_storage_dir = temp_storage_dir("persisted-snapshot-restore");
    let repo = temp_repo_dir("persisted-snapshot-repo");
    write_indexable_repo(&repo, "persisted_snapshot_symbol");

    let server = test_server(&initial_storage_dir);
    let initial_index = server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
    assert_eq!(initial_index["status"], "done");
    let normalized_repo = normalize_repo_dir(repo.to_string_lossy().as_ref());
    let project_storage = contextro_config::project_storage_dir(&normalized_repo);
    assert!(!contextro_indexing::load_hashes(&project_storage).is_empty());
    assert!(server
        .state
        .load_persisted_repo_snapshot(&normalized_repo)
        .is_some());
    let initial_bm25 = server.state.active_bm25();

    let restored_server = test_server(&restore_storage_dir);
    let restored =
        restored_server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
    assert_eq!(restored["status"], "done");
    assert_eq!(restored["index_mode"], "restored");
    assert_eq!(restored["restored_from_cache"], true);
    assert_eq!(
        restored["message"],
        "Restored from persisted repo snapshot."
    );
    assert!(restored["restore_ms"].as_f64().unwrap_or(0.0) >= 0.0);
    assert!(restored["request_ms"].as_f64().unwrap_or(0.0) >= 0.0);

    let search = restored_server.handle_search(&json!({"query": "persisted_snapshot_symbol"}));
    let results = search["results"].as_array().expect("results array");
    assert!(results
        .iter()
        .any(|result| result["name"] == "persisted_snapshot_symbol"));
    assert!(std::sync::Arc::ptr_eq(
        &initial_bm25,
        &restored_server.state.active_bm25()
    ));

    let _ = std::fs::remove_dir_all(repo.clone());
    let _ = std::fs::remove_dir_all(initial_storage_dir);
    let _ = std::fs::remove_dir_all(restore_storage_dir);
    let _ = std::fs::remove_dir_all(project_storage);
}

#[test]
fn test_handle_index_rebuilds_when_persisted_snapshot_is_stale() {
    let initial_storage_dir = temp_storage_dir("persisted-snapshot-stale-source");
    let restore_storage_dir = temp_storage_dir("persisted-snapshot-stale-restore");
    let repo = temp_repo_dir("persisted-snapshot-stale-repo");
    write_indexable_repo(&repo, "stale_snapshot_symbol");

    let server = test_server(&initial_storage_dir);
    let initial_index = server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
    assert_eq!(initial_index["status"], "done");

    write_indexable_repo(&repo, "fresh_snapshot_symbol");

    let restored_server = test_server(&restore_storage_dir);
    let restored =
        restored_server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
    assert_eq!(restored["status"], "done");
    assert_eq!(restored["index_mode"], "fresh");
    assert_ne!(restored.get("restored_from_cache"), Some(&json!(true)));
    assert!(restored["graph_ms"].as_f64().unwrap_or(0.0) >= 0.0);
    assert!(restored["request_ms"].as_f64().unwrap_or(0.0) >= 0.0);

    let fresh_search = restored_server.handle_search(&json!({"query": "fresh_snapshot_symbol"}));
    let fresh_results = fresh_search["results"].as_array().expect("results array");
    assert!(fresh_results
        .iter()
        .any(|result| result["name"] == "fresh_snapshot_symbol"));

    let stale_search = restored_server.handle_search(&json!({"query": "stale_snapshot_symbol"}));
    let stale_results = stale_search["results"].as_array().expect("results array");
    assert!(!stale_results
        .iter()
        .any(|result| result["name"] == "stale_snapshot_symbol"));

    let _ = std::fs::remove_dir_all(repo.clone());
    let _ = std::fs::remove_dir_all(initial_storage_dir);
    let _ = std::fs::remove_dir_all(restore_storage_dir);
    let _ = std::fs::remove_dir_all(contextro_config::project_storage_dir(&normalize_repo_dir(
        repo.to_string_lossy().as_ref(),
    )));
}

#[test]
fn test_restore_repo_snapshot_falls_back_when_graph_snapshot_is_missing() {
    let storage_dir = temp_storage_dir("persisted-snapshot-legacy-graph");
    let repo = temp_repo_dir("persisted-snapshot-legacy-repo");
    write_indexable_repo(&repo, "legacy_snapshot_symbol");

    let server = test_server(&storage_dir);
    let indexed = server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
    assert_eq!(indexed["status"], "done");

    let normalized_repo = normalize_repo_dir(repo.to_string_lossy().as_ref());
    let mut snapshot = server
        .state
        .load_persisted_repo_snapshot(&normalized_repo)
        .expect("snapshot should exist");
    snapshot.graph = contextro_engines::graph::GraphSnapshot::default();

    server.clear_active_scope();
    let restored = server.restore_repo_snapshot(&normalized_repo, &snapshot).0;
    assert_eq!(restored["status"], "done");
    assert_eq!(restored["restored_from_cache"], true);

    let search = server.handle_search(&json!({"query": "legacy_snapshot_symbol"}));
    let results = search["results"].as_array().expect("results array");
    assert!(results
        .iter()
        .any(|result| result["name"] == "legacy_snapshot_symbol"));

    let _ = std::fs::remove_dir_all(repo.clone());
    let _ = std::fs::remove_dir_all(storage_dir);
    let _ = std::fs::remove_dir_all(contextro_config::project_storage_dir(&normalized_repo));
}
