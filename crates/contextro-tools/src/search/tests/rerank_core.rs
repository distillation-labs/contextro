use super::*;

#[test]
fn test_symbol_query_guard_drops_partial_noise_matches() {
    let filtered = apply_symbol_query_guard(
        "zzzzzzzzzz_no_match_expected",
        vec![
            make_named_result(
                "noise-1",
                "match_url_with_domain_pattern",
                "traverse/utils.py",
                0.8,
                &["bm25"],
            ),
            make_named_result(
                "noise-2",
                "test_no_retry_on_400",
                "tests/ci/test_llm_retries.py",
                0.7,
                &["bm25"],
            ),
        ],
    );

    assert!(filtered.is_empty());
}

#[test]
fn test_symbol_query_guard_keeps_full_identifier_matches() {
    let filtered = apply_symbol_query_guard(
        "browser_session",
        vec![
            make_named_result(
                "browser-session",
                "BrowserSession",
                "traverse/browser/session.py",
                0.9,
                &["bm25"],
            ),
            make_named_result(
                "session-only",
                "attach_handler_to_session",
                "traverse/browser/watchdog_base.py",
                0.7,
                &["bm25"],
            ),
        ],
    );

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].symbol_name, "BrowserSession");
}

#[test]
fn test_natural_language_reranker_prefers_implementation_over_tests() {
    let reranked = rerank_natural_language_results(
        "security watchdog domain filtering",
        vec![
            make_named_result(
                "test-hit",
                "test_is_root_domain_helper",
                "tests/ci/security/test_domain_filtering.py",
                0.73,
                &["bm25"],
            ),
            make_named_result(
                "impl-hit",
                "SecurityWatchdog._is_root_domain",
                "traverse/browser/watchdogs/security_watchdog.py",
                0.64,
                &["vector"],
            ),
        ],
    );

    assert_eq!(reranked[0].symbol_name, "SecurityWatchdog._is_root_domain");
}

#[test]
fn test_natural_language_reranker_skips_explicit_test_queries() {
    let reranked = rerank_natural_language_results(
        "test domain filtering fixtures",
        vec![
            make_named_result(
                "test-hit",
                "test_is_root_domain_helper",
                "tests/ci/security/test_domain_filtering.py",
                0.73,
                &["bm25"],
            ),
            make_named_result(
                "impl-hit",
                "SecurityWatchdog._is_root_domain",
                "traverse/browser/watchdogs/security_watchdog.py",
                0.64,
                &["vector"],
            ),
        ],
    );

    assert_eq!(reranked[0].symbol_name, "test_is_root_domain_helper");
}

#[test]
fn test_natural_language_reranker_demotes_test_symbols_inside_src_files() {
    let reranked = rerank_natural_language_results(
        "semantic search ranking noise",
        vec![
            make_named_result(
                "test-hit",
                "test_symbol_query_guard_drops_partial_noise_matches",
                "crates/contextro-tools/src/search.rs",
                0.74,
                &["bm25"],
            ),
            make_named_result(
                "impl-hit",
                "handle_search",
                "crates/contextro-tools/src/search.rs",
                0.70,
                &["bm25", "vector"],
            ),
            make_named_result(
                "engine-hit",
                "execute_search",
                "crates/contextro-engines/src/search.rs",
                0.68,
                &["bm25"],
            ),
        ],
    );

    assert_ne!(
        reranked[0].symbol_name,
        "test_symbol_query_guard_drops_partial_noise_matches"
    );
    assert_eq!(reranked[0].symbol_name, "handle_search");
}

#[test]
fn test_natural_language_reranker_prefers_handler_over_test_scaffolding() {
    let reranked = rerank_natural_language_results(
        "repo_add auto indexes",
        vec![
            make_named_result(
                "test-hit",
                "test_knowledge_add_indexes_nested_directory_contents",
                "crates/contextro-tools/src/memory.rs",
                0.73,
                &["bm25"],
            ),
            make_named_result(
                "impl-hit",
                "handle_repo_add",
                "crates/contextro-tools/src/git_tools.rs",
                0.68,
                &["bm25", "vector"],
            ),
        ],
    );

    assert_eq!(reranked[0].symbol_name, "handle_repo_add");
}

#[test]
fn test_natural_language_reranker_demotes_internal_helper_symbols() {
    let mut helper = make_named_result(
        "helper-hit",
        "hybrid_candidate_limit",
        "crates/contextro-tools/src/search.rs",
        0.78,
        &["bm25"],
    );
    helper.signature = "fn hybrid_candidate_limit(query: &str, limit: usize) -> usize".into();

    let mut entrypoint = make_named_result(
        "entrypoint-hit",
        "handle_search",
        "crates/contextro-tools/src/search.rs",
        0.64,
        &["bm25", "vector"],
    );
    entrypoint.signature = "pub fn handle_search(args: &Value) -> Value".into();

    let reranked =
        rerank_natural_language_results("hybrid search ranking", vec![helper, entrypoint]);

    assert_eq!(reranked[0].symbol_name, "handle_search");
}

#[test]
fn test_natural_language_reranker_uses_code_overlap_to_demote_engine_noise() {
    let mut engine = make_named_result(
        "engine-hit",
        "Bm25Engine.search",
        "crates/contextro-engines/src/bm25.rs",
        0.74,
        &["bm25"],
    );
    engine.signature = "pub fn search(&self, query: &str) -> Vec<SearchResult>".into();
    engine.code =
        "pub fn search(&self, query: &str) -> Vec<SearchResult> { self.index.search(query) }"
            .into();

    let mut handler = make_named_result(
        "handler-hit",
        "handle_search",
        "crates/contextro-tools/src/search.rs",
        0.68,
        &["bm25", "vector"],
    );
    handler.signature = "pub fn handle_search(args: &Value) -> Value".into();
    handler.code = r#"match mode.as_str() {
    "hybrid" => fuse_results(query, core_results, vec_results, limit),
    _ => execute_search(&options, bm25, graph, cache, &fusion).results,
}"#
    .into();

    let reranked = rerank_natural_language_results("hybrid search ranking", vec![engine, handler]);
    assert_eq!(reranked[0].symbol_name, "handle_search");
}

#[test]
fn test_natural_language_reranker_prefers_tool_surface_for_quality_queries() {
    let mut engine = make_named_result(
        "engine-hit",
        "execute_search",
        "crates/contextro-engines/src/search.rs",
        0.80,
        &["bm25", "graph"],
    );
    engine.signature = "pub fn execute_search(options: &SearchOptions) -> SearchResponse".into();
    engine.code =
        "let results = fusion.fuse(&ranked_lists); apply_graph_consensus(&mut results, graph);"
            .into();

    let mut handler = make_named_result(
        "handler-hit",
        "handle_search",
        "crates/contextro-tools/src/search.rs",
        0.58,
        &["bm25", "vector"],
    );
    handler.signature = "pub fn handle_search(args: &Value) -> Value".into();
    handler.code = r#"match mode.as_str() {
    "hybrid" => fuse_results(query, core_results, vec_results, limit),
    _ => execute_search(&options, bm25, graph, cache, &fusion).results,
}"#
    .into();

    let reranked =
        rerank_natural_language_results("semantic search ranking noise", vec![engine, handler]);

    assert_eq!(reranked[0].symbol_name, "handle_search");
}

#[test]
fn test_natural_language_reranker_prefers_grounded_subsystem_results_over_vector_noise() {
    let mut chart_style = make_named_result(
        "chart-style",
        "ChartStyle",
        "packages/charts/src/chart_style.ts",
        0.82,
        &["vector"],
    );
    chart_style.signature = "export interface ChartStyle".into();
    chart_style.code = "export interface ChartStyle { palette: string[] }".into();

    let mut plugin_helper = make_named_result(
        "plugin-helper",
        "getAndroidManifestPluginHelpers",
        "packages/mobile/src/android/manifest/plugin_helpers.ts",
        0.79,
        &["vector"],
    );
    plugin_helper.signature = "export function getAndroidManifestPluginHelpers(): Helpers".into();
    plugin_helper.code = "return { withAndroidManifest, withManifestPlugin };".into();

    let mut observability_server = make_named_result(
        "observability-server",
        "startObservabilityServer",
        "packages/observability/src/server.ts",
        0.60,
        &["bm25", "vector"],
    );
    observability_server.signature =
        "export function startObservabilityServer(config: ObservabilityConfig)".into();
    observability_server.code =
        "const config = loadObservabilityConfig(); return createServer(config);".into();

    let mut observability_sentry = make_named_result(
        "observability-sentry",
        "initSentry",
        "packages/observability/src/sentry.ts",
        0.58,
        &["bm25"],
    );
    observability_sentry.signature =
        "export function initSentry(config: ObservabilityConfig)".into();
    observability_sentry.code = "Sentry.init({ dsn: config.dsn });".into();

    let reranked = rerank_natural_language_results(
        "observability config",
        vec![
            chart_style,
            plugin_helper,
            observability_server,
            observability_sentry,
        ],
    );

    assert!(reranked[0]
        .filepath
        .starts_with("packages/observability/src/"));
    assert!(reranked[1]
        .filepath
        .starts_with("packages/observability/src/"));
    assert!(reranked
        .iter()
        .take(2)
        .all(|result| result.filepath.contains("observability")));
}

#[test]
fn test_natural_language_query_terms_remove_stopwords_and_normalize_cache_forms() {
    let terms = natural_language_query_terms("how does caching work with configurations");

    assert!(terms.contains(&"cache".into()));
    assert!(terms.contains(&"config".into()));
    assert!(!terms.contains(&"how".into()));
    assert!(!terms.contains(&"work".into()));
}

#[test]
fn test_result_query_overlap_matches_cache_and_configuration_variants() {
    let mut result = make_named_result(
        "cache-config",
        "QueryCache",
        "crates/contextro-engines/src/cache.rs",
        0.70,
        &["bm25"],
    );
    result.signature = "pub struct QueryCacheConfig".into();
    result.code = "Caches search responses using configuration-driven TTL eviction.".into();

    let overlap = result_query_overlap(&natural_language_query_terms("caching config"), &result);
    assert_eq!(overlap, 1.0);
}

#[test]
fn test_cache_queries_do_not_target_product_surface_bias() {
    assert!(!query_targets_product_surface(
        "how does the query cache work, TTL eviction"
    ));
    assert!(!query_targets_product_surface("how does caching work"));
}

#[test]
fn test_engine_internal_classifier_recognizes_cache_infra_results() {
    let cache = make_named_result(
        "cache-hit",
        "QueryCache",
        "crates/contextro-engines/src/cache.rs",
        0.72,
        &["bm25"],
    );
    let ttl = make_named_result(
        "ttl-hit",
        "evict_expired_entries",
        "crates/contextro-engines/src/cache.rs",
        0.61,
        &["bm25"],
    );

    assert!(is_probable_engine_internal_search_result(&cache));
    assert!(is_probable_engine_internal_search_result(&ttl));
}

#[test]
fn test_meta_support_classifier_catches_plugin_setup_and_stub_noise() {
    let plugin = make_named_result(
        "plugin-hit",
        "withAndroidManifestFixes",
        "apps/mobile/plugins/with-android-manifest-fixes.ts",
        0.81,
        &["vector"],
    );
    let setup = make_named_result(
        "setup-hit",
        "setupWorkspaceObservability",
        "scripts/agents/setup-workspace-observability.ts",
        0.83,
        &["vector"],
    );
    let stub = make_named_result(
        "stub-hit",
        "createColumnStub",
        "tests/helpers/create_column_stub.ts",
        0.76,
        &["bm25"],
    );

    assert!(is_probable_meta_support_result(&plugin));
    assert!(is_probable_meta_support_result(&setup));
    assert!(is_probable_meta_support_result(&stub));
}

#[test]
fn test_observability_queries_target_engine_internals() {
    assert!(query_targets_engine_internals("observability config"));
    assert!(query_targets_engine_internals(
        "how does observability configuration work"
    ));
}

#[test]
fn test_support_or_tooling_intent_detects_setup_and_plugin_queries() {
    assert!(query_targets_support_or_tooling(
        "plugin setup path helpers"
    ));
    assert!(!query_targets_support_or_tooling("observability config"));
}
