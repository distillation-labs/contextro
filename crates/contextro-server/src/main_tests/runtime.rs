use super::*;

#[test]
fn test_restart_restores_active_scope_and_search_after_repo_add() {
    let storage_dir = temp_storage_dir("restart-repo-add");
    let repo = temp_repo_dir("restart-repo-a");
    write_indexable_repo(&repo, "restart_repo_symbol");

    let server = test_server(&storage_dir);
    let add_result = server.dispatch(
        "repo_add",
        json!({"path": repo.to_string_lossy().to_string()}),
    );
    assert_ne!(add_result.is_error, Some(true));

    let restarted = test_server(&storage_dir);
    assert!(*restarted.state.indexed.read());
    assert_eq!(
        restarted
            .state
            .codebase_path
            .read()
            .clone()
            .map(|path| normalize_repo_dir(&path)),
        Some(normalize_repo_dir(repo.to_string_lossy().as_ref()))
    );

    let search = restarted.handle_search(&json!({"query": "restart_repo_symbol"}));
    let results = search["results"].as_array().expect("results array");
    assert!(results
        .iter()
        .any(|result| result["name"] == "restart_repo_symbol"));

    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(storage_dir);
}

#[test]
fn test_repo_add_dispatch_replaces_stale_git_hint_after_auto_index() {
    let storage_dir = temp_storage_dir("repo-add-hint");
    let server = test_server(&storage_dir);
    let repo = temp_repo_dir("repo-add-hint-repo");
    write_indexable_repo(&repo, "repo_add_hint_symbol");
    let git_init = std::process::Command::new("git")
        .arg("init")
        .arg(&repo)
        .output()
        .expect("initialize git repo");
    assert!(git_init.status.success());

    let result = server.dispatch(
        "repo_add",
        json!({"path": repo.to_string_lossy().to_string()}),
    );
    assert_ne!(result.is_error, Some(true));

    let content = serde_json::to_value(&result.content[0]).expect("serialize tool content");
    let text = content["text"].as_str().expect("tool text payload");
    let payload: Value = serde_json::from_str(text).expect("repo_add JSON payload");

    assert_eq!(payload["registered"], true);
    assert_eq!(payload["indexed"], true);
    assert_eq!(
        payload["hint"],
        "Repository registered, indexed, and set as the active repo scope."
    );
    assert!(!text.contains("Run index(path) to build the graph and enable search for this repo."));

    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(storage_dir);
}

#[test]
fn test_index_skips_vector_index_for_single_chunk_repo() {
    let storage_dir = temp_storage_dir("single-chunk-vector-skip");
    let server = test_server(&storage_dir);
    let repo = temp_repo_dir("single-chunk-repo");
    write_indexable_repo(&repo, "single_chunk_symbol");

    let result = server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));

    assert_eq!(result["status"], "done");
    assert_eq!(result["index_mode"], "fresh");
    assert_eq!(result["total_chunks"], 1);
    assert_eq!(result["vector_chunks"], 0);

    let search = server.handle_search(&json!({"query": "single_chunk_symbol"}));
    let results = search["results"].as_array().expect("results array");
    assert!(results
        .iter()
        .any(|entry| entry["name"] == "single_chunk_symbol"));

    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(storage_dir);
}

#[test]
fn test_handle_index_skips_reindex_for_unchanged_loaded_repo() {
    let storage_dir = temp_storage_dir("index-skip-unchanged");
    let server = test_server(&storage_dir);
    let repo = temp_repo_dir("index-skip-unchanged-repo");
    write_indexable_repo(&repo, "skip_unchanged_symbol");

    let indexed = server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
    assert_eq!(indexed["status"], "done");
    assert_eq!(indexed["index_mode"], "fresh");

    let skipped = server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
    assert_eq!(skipped["status"], "done");
    assert_eq!(skipped["index_mode"], "skipped");
    assert_eq!(skipped["message"], "No files changed since last index.");
    assert!(skipped["request_ms"].as_f64().unwrap_or(0.0) >= 0.0);

    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(storage_dir);
}

#[test]
fn test_restart_repo_remove_restores_previous_scope() {
    let storage_dir = temp_storage_dir("restart-repo-restore");
    let repo_a = temp_repo_dir("restart-restore-a");
    let repo_b = temp_repo_dir("restart-restore-b");
    write_indexable_repo(&repo_a, "restore_repo_a_symbol");
    write_indexable_repo(&repo_b, "restore_repo_b_symbol");

    let server = test_server(&storage_dir);
    let index_a = server.handle_index(&json!({"path": repo_a.to_string_lossy().to_string()}));
    assert_eq!(index_a["status"], "done");
    let add_b = server.dispatch(
        "repo_add",
        json!({"path": repo_b.to_string_lossy().to_string()}),
    );
    assert_ne!(add_b.is_error, Some(true));

    let restarted = test_server(&storage_dir);
    let remove_result =
        restarted.handle_repo_remove(&json!({"path": repo_b.to_string_lossy().to_string()}));
    assert_eq!(remove_result["removed"], true);
    assert_eq!(remove_result["active_scope_restored"], true);
    assert_eq!(
        restarted
            .state
            .codebase_path
            .read()
            .clone()
            .map(|path| normalize_repo_dir(&path)),
        Some(normalize_repo_dir(repo_a.to_string_lossy().as_ref()))
    );

    let search = restarted.handle_search(&json!({"query": "restore_repo_a_symbol"}));
    let results = search["results"].as_array().expect("results array");
    assert!(results
        .iter()
        .any(|result| result["name"] == "restore_repo_a_symbol"));

    let _ = std::fs::remove_dir_all(repo_a);
    let _ = std::fs::remove_dir_all(repo_b);
    let _ = std::fs::remove_dir_all(storage_dir);
}

#[test]
fn test_restart_repo_remove_only_active_repo_clears_persisted_scope() {
    let storage_dir = temp_storage_dir("restart-repo-clear");
    let repo = temp_repo_dir("restart-clear-a");
    write_indexable_repo(&repo, "restart_clear_symbol");

    let server = test_server(&storage_dir);
    let add_result = server.dispatch(
        "repo_add",
        json!({"path": repo.to_string_lossy().to_string()}),
    );
    assert_ne!(add_result.is_error, Some(true));

    let restarted = test_server(&storage_dir);
    let remove_result =
        restarted.handle_repo_remove(&json!({"path": repo.to_string_lossy().to_string()}));
    assert_eq!(remove_result["removed"], true);
    assert_eq!(remove_result["active_scope_cleared"], true);
    assert!(!(*restarted.state.indexed.read()));
    assert_eq!(*restarted.state.codebase_path.read(), None);
    assert!(!storage_dir.join("repo-scope.json").exists());

    let restarted_again = test_server(&storage_dir);
    assert!(!(*restarted_again.state.indexed.read()));
    assert_eq!(*restarted_again.state.codebase_path.read(), None);

    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(storage_dir);
}

#[test]
fn test_search_returns_clear_error_when_no_codebase_loaded() {
    let storage_dir = temp_storage_dir("search-empty-state");
    let server = test_server(&storage_dir);

    let result = server.handle_search(&json!({"query": "anything"}));

    assert_eq!(
        result["error"],
        "No codebase loaded. Run 'index(path)' or 'repo_add(path)' to load an active repo scope."
    );

    let _ = std::fs::remove_dir_all(storage_dir);
}

#[test]
fn test_graph_analysis_dispatch_reuses_cached_response_across_max_tokens_variants() {
    let storage_dir = temp_storage_dir("analysis-response-cache");
    let server = test_server(&storage_dir);
    let repo = temp_repo_dir("analysis-response-cache-repo");
    write_indexable_repo(&repo, "cached_analysis_symbol");

    let indexed = server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
    assert_eq!(indexed["status"], "done");

    let cache_hit_rate_before = server.state.query_cache.hit_rate();
    let session_total_before = contextro_tools::session::handle_session_snapshot(
        &json!({"limit": 20, "type": "overview"}),
        &server.state.session_tracker,
    )["total"]
        .as_u64()
        .unwrap_or(0);
    let first = server.dispatch("overview", json!({"max_tokens": 1}));
    assert_ne!(first.is_error, Some(true));

    let second = server.dispatch("overview", json!({"max_tokens": 50}));
    assert_ne!(second.is_error, Some(true));
    assert!(
        server.state.query_cache.hit_rate() > cache_hit_rate_before,
        "expected cached overview response to increase hit rate"
    );
    let session_total_after = contextro_tools::session::handle_session_snapshot(
        &json!({"limit": 20, "type": "overview"}),
        &server.state.session_tracker,
    )["total"]
        .as_u64()
        .unwrap_or(0);
    assert_eq!(session_total_after, session_total_before + 2);

    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(storage_dir);
}

#[test]
fn test_graph_analysis_cache_invalidates_after_reindex() {
    let storage_dir = temp_storage_dir("analysis-response-cache-invalidation");
    let server = test_server(&storage_dir);
    let repo = temp_repo_dir("analysis-response-cache-invalidation-repo");
    write_indexable_repo(&repo, "cache_before_symbol");

    let indexed = server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
    assert_eq!(indexed["status"], "done");

    let first = server.dispatch("overview", json!({}));
    assert_ne!(first.is_error, Some(true));
    let hit_rate_before = server.state.query_cache.hit_rate();

    write_indexable_repo(&repo, "cache_after_symbol");
    let reindexed = server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
    assert_eq!(reindexed["status"], "done");

    let second = server.dispatch("overview", json!({}));
    assert_ne!(second.is_error, Some(true));
    assert_eq!(server.state.query_cache.hit_rate(), hit_rate_before);

    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(storage_dir);
}
