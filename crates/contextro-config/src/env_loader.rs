use super::Settings;

pub(super) fn settings_from_env() -> Settings {
    let mut settings = Settings::default();

    macro_rules! env_str {
        ($var:expr, $field:ident) => {
            if let Ok(value) = std::env::var($var) {
                settings.$field = value;
            }
        };
    }

    macro_rules! env_parsed {
        ($var:expr, $field:ident, $ty:ty) => {
            if let Ok(value) = std::env::var($var) {
                if let Ok(parsed) = value.parse::<$ty>() {
                    settings.$field = parsed;
                }
            }
        };
    }

    macro_rules! env_bool {
        ($var:expr, $field:ident) => {
            if let Ok(value) = std::env::var($var) {
                settings.$field = matches!(value.to_lowercase().as_str(), "true" | "1" | "yes");
            }
        };
    }

    env_str!("CTX_STORAGE_DIR", storage_dir);

    env_str!("CTX_EMBEDDING_MODEL", embedding_model);
    env_str!("CTX_EMBEDDING_DEVICE", embedding_device);
    env_parsed!("CTX_EMBEDDING_BATCH_SIZE", embedding_batch_size, usize);

    env_parsed!("CTX_MAX_FILE_SIZE_MB", max_file_size_mb, usize);
    if let Ok(value) = std::env::var("CTX_MAX_WORKERS") {
        settings.max_workers = value.parse::<usize>().ok();
    }
    env_parsed!("CTX_CHUNK_MAX_CHARS", chunk_max_chars, usize);
    env_str!("CTX_CHUNK_CONTEXT_MODE", chunk_context_mode);
    env_parsed!(
        "CTX_CHUNK_CONTEXT_PATH_DEPTH",
        chunk_context_path_depth,
        usize
    );
    env_parsed!("CTX_INDEX_FILE_BATCH_SIZE", index_file_batch_size, usize);
    env_bool!("CTX_SKIP_ASTGREP", skip_astgrep);
    env_bool!(
        "CTX_SMART_CHUNK_RELATIONSHIPS_ENABLED",
        smart_chunk_relationships_enabled
    );
    env_bool!(
        "CTX_SMART_CHUNK_FILE_CONTEXT_ENABLED",
        smart_chunk_file_context_enabled
    );
    env_bool!(
        "CTX_INCREMENTAL_INDEX_FAST_PATH_ENABLED",
        incremental_index_fast_path_enabled
    );

    env_parsed!("CTX_GRAPH_MAX_DEPTH", graph_max_depth, usize);

    env_str!("CTX_SEARCH_MODE", search_mode);
    env_str!("CTX_RERANKER_MODEL", reranker_model);
    env_parsed!("CTX_FUSION_WEIGHT_VECTOR", fusion_weight_vector, f64);
    env_parsed!("CTX_FUSION_WEIGHT_BM25", fusion_weight_bm25, f64);
    env_parsed!("CTX_FUSION_WEIGHT_GRAPH", fusion_weight_graph, f64);
    env_parsed!("CTX_RELEVANCE_THRESHOLD", relevance_threshold, f64);
    env_parsed!("CTX_SEARCH_CACHE_MAX_SIZE", search_cache_max_size, usize);
    env_parsed!(
        "CTX_SEARCH_CACHE_SIMILARITY_THRESHOLD",
        search_cache_similarity_threshold,
        f64
    );
    env_parsed!(
        "CTX_SEARCH_CACHE_TTL_SECONDS",
        search_cache_ttl_seconds,
        f64
    );
    env_parsed!(
        "CTX_SEARCH_SANDBOX_THRESHOLD_TOKENS",
        search_sandbox_threshold_tokens,
        usize
    );
    env_parsed!(
        "CTX_SEARCH_SANDBOX_MAX_ENTRIES",
        search_sandbox_max_entries,
        usize
    );
    env_parsed!(
        "CTX_SEARCH_SANDBOX_TTL_SECONDS",
        search_sandbox_ttl_seconds,
        f64
    );
    env_parsed!("CTX_SEARCH_PREVIEW_RESULTS", search_preview_results, usize);
    env_parsed!(
        "CTX_SEARCH_PREVIEW_CODE_CHARS",
        search_preview_code_chars,
        usize
    );
    env_bool!(
        "CTX_SEARCH_ADAPTIVE_RESULT_COUNT_ENABLED",
        search_adaptive_result_count_enabled
    );
    env_parsed!(
        "CTX_SEARCH_ADAPTIVE_HIGH_CONFIDENCE_LIMIT",
        search_adaptive_high_confidence_limit,
        usize
    );
    env_parsed!(
        "CTX_SEARCH_ADAPTIVE_MEDIUM_CONFIDENCE_LIMIT",
        search_adaptive_medium_confidence_limit,
        usize
    );
    env_parsed!(
        "CTX_SEARCH_CODE_BUDGET_TOP_CHARS",
        search_code_budget_top_chars,
        usize
    );
    env_parsed!(
        "CTX_SEARCH_CODE_BUDGET_SECOND_CHARS",
        search_code_budget_second_chars,
        usize
    );
    env_parsed!(
        "CTX_SEARCH_CODE_BUDGET_TAIL_CHARS",
        search_code_budget_tail_chars,
        usize
    );
    env_bool!("CTX_SEARCH_PREWARM_ENABLED", search_prewarm_enabled);

    env_parsed!("CTX_MAX_MEMORY_MB", max_memory_mb, usize);

    env_str!("CTX_OUTPUT_FORMAT", output_format);

    env_str!("CTX_LOG_LEVEL", log_level);

    env_bool!("CTX_TRUST_REMOTE_CODE", trust_remote_code);
    env_str!("CTX_PERMISSION_LEVEL", default_permission_level);

    env_bool!("CTX_AUTO_WARM_START", auto_warm_start);

    env_bool!("CTX_AUDIT_ENABLED", audit_enabled);

    env_bool!("CTX_RATE_LIMIT_ENABLED", rate_limit_enabled);
    env_parsed!("CTX_RATE_LIMIT_DEFAULT_RATE", rate_limit_default_rate, f64);
    env_parsed!(
        "CTX_RATE_LIMIT_DEFAULT_BURST",
        rate_limit_default_burst,
        usize
    );

    env_bool!("CTX_COMMIT_HISTORY_ENABLED", commit_history_enabled);
    env_parsed!("CTX_COMMIT_HISTORY_LIMIT", commit_history_limit, usize);
    env_str!("CTX_COMMIT_HISTORY_SINCE", commit_history_since);
    env_bool!("CTX_REALTIME_INDEXING_ENABLED", realtime_indexing_enabled);
    env_parsed!("CTX_BRANCH_POLL_INTERVAL", branch_poll_interval_secs, f64);
    env_parsed!(
        "CTX_REINDEX_DEBOUNCE_SECONDS",
        reindex_debounce_seconds,
        f64
    );
    env_bool!("CTX_FILE_WATCHER_ENABLED", file_watcher_enabled);

    env_bool!("CTX_CROSS_REPO_ENABLED", cross_repo_enabled);
    env_str!("CTX_CROSS_REPO_PATHS", cross_repo_paths);

    env_str!("CTX_TRANSPORT", transport);
    env_str!("CTX_HTTP_HOST", http_host);
    env_parsed!("CTX_HTTP_PORT", http_port, u16);

    settings
}
