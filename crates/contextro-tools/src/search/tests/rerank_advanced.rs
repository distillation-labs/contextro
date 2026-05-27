use super::*;

#[test]
fn test_natural_language_reranker_prefers_observability_builders_over_generic_state_symbols() {
    let mut settlement_state = make_named_result(
        "settlement-state",
        "SettlementSyncFormState",
        "apps/app/src/features/finance/accounting/components/accounting-ap-sync.utils.ts",
        0.97,
        &["vector"],
    );
    settlement_state.signature =
        "export type SettlementSyncFormState = typeof DEFAULT_SETTLEMENT_SYNC_CONFIG".into();
    settlement_state.code = "settlement sync config state for accounting forms".into();

    let mut observability_builder = make_named_result(
        "obs-builder",
        "buildObservabilityConfig",
        "packages/observability/src/server.ts",
        0.63,
        &["bm25", "vector"],
    );
    observability_builder.signature =
        "export function buildObservabilityConfig(serviceName: string): ObservabilityConfig".into();
    observability_builder.code =
        "build observability config with telemetry and tracing exporters".into();

    let mut sentry_builder = make_named_result(
        "sentry-builder",
        "buildNextSentryConfigOptions",
        "packages/observability/src/sentry.ts",
        0.61,
        &["bm25"],
    );
    sentry_builder.signature =
        "export function buildNextSentryConfigOptions(): NextSentryBuildOptions | null".into();
    sentry_builder.code = "build sentry config options for observability".into();

    let reranked = rerank_natural_language_results(
        "observability config",
        vec![settlement_state, observability_builder, sentry_builder],
    );

    assert_eq!(reranked[0].symbol_name, "buildObservabilityConfig");
    assert_eq!(reranked[1].symbol_name, "buildNextSentryConfigOptions");
}

#[test]
fn test_natural_language_reranker_prefers_query_cache_over_read_write_cache_helpers() {
    let mut read_cache = make_named_result(
        "read-cache",
        "read_cache",
        "crates/contextro-server/src/update_check.rs",
        0.96,
        &["bm25"],
    );
    read_cache.signature = "fn read_cache(path: &Path) -> Option<String>".into();
    read_cache.code = "reads update cache from disk".into();

    let mut write_cache = make_named_result(
        "write-cache",
        "write_cache",
        "crates/contextro-server/src/update_check.rs",
        0.94,
        &["bm25"],
    );
    write_cache.signature = "fn write_cache(path: &Path, version: &str)".into();
    write_cache.code = "writes update cache to disk".into();

    let mut find_hf_cache_path = make_named_result(
        "hf-cache-path",
        "find_hf_cache_path",
        "crates/contextro-indexing/src/embedding.rs",
        0.92,
        &["vector"],
    );
    find_hf_cache_path.signature = "fn find_hf_cache_path(hf_id: &str) -> Option<String>".into();
    find_hf_cache_path.code = "find huggingface cache path".into();

    let mut query_cache = make_named_result(
        "query-cache",
        "QueryCache",
        "crates/contextro-engines/src/cache.rs",
        0.61,
        &["bm25", "vector"],
    );
    query_cache.signature = "pub struct QueryCache".into();
    query_cache.code = "stores cached search responses with TTL eviction and invalidation".into();

    let mut query_cache_get = make_named_result(
        "query-cache-get",
        "QueryCache::get",
        "crates/contextro-engines/src/cache.rs",
        0.58,
        &["bm25"],
    );
    query_cache_get.signature = "pub fn get(&self, query: &str) -> Option<SearchResponse>".into();
    query_cache_get.code = "returns cached search responses when entries have not expired".into();

    let reranked = rerank_natural_language_results(
        "how does caching work",
        vec![
            read_cache,
            write_cache,
            find_hf_cache_path,
            query_cache,
            query_cache_get,
        ],
    );

    assert_eq!(reranked[0].symbol_name, "QueryCache");
    assert_eq!(reranked[1].symbol_name, "QueryCache::get");
}

#[test]
fn test_natural_language_reranker_penalizes_observability_support_noise() {
    let mut setup = make_named_result(
        "setup-hit",
        "setupWorkspaceObservability",
        "scripts/agents/setup-workspace-observability.ts",
        1.018,
        &["vector"],
    );
    setup.signature = "export function setupWorkspaceObservability()".into();
    setup.code = "setup workspace observability tooling and scripts".into();

    let mut plugin = make_named_result(
        "plugin-hit",
        "withAndroidManifestFixes",
        "apps/mobile/plugins/with-android-manifest-fixes.ts",
        1.0139,
        &["vector"],
    );
    plugin.signature = "export function withAndroidManifestFixes(config: ConfigPlugin)".into();
    plugin.code = "android manifest plugin helper".into();

    let mut chart = make_named_result(
        "chart-hit",
        "ChartStyle",
        "apps/web/src/components/ui/chart.tsx",
        0.924,
        &["vector"],
    );
    chart.signature = "export interface ChartStyle".into();
    chart.code = "chart styling tokens".into();

    let mut config = make_named_result(
        "config-hit",
        "ObservabilityConfig",
        "packages/observability/src/config.ts",
        0.66,
        &["bm25", "vector"],
    );
    config.signature = "export interface ObservabilityConfig".into();
    config.code = "load observability config, exporters, telemetry, and tracing".into();

    let mut init = make_named_result(
        "init-hit",
        "initializeObservability",
        "packages/observability/src/index.ts",
        0.63,
        &["bm25"],
    );
    init.signature = "export function initializeObservability(config: ObservabilityConfig)".into();
    init.code = "initialize telemetry using observability config".into();

    let reranked = rerank_natural_language_results(
        "observability config",
        vec![setup, plugin, chart, config, init],
    );

    assert_eq!(reranked[0].filepath, "packages/observability/src/config.ts");
    assert_eq!(reranked[1].filepath, "packages/observability/src/index.ts");
    assert!(reranked
        .iter()
        .take(2)
        .all(|result| !is_probable_meta_support_result(result)));
}

#[test]
fn test_natural_language_reranker_prefers_cache_implementation_over_cache_tests_and_helpers() {
    let mut cache_tests = make_named_result(
        "cache-tests",
        "test_handle_search_surfaces_query_cache_for_caching_queries",
        "crates/contextro-tools/src/search.rs",
        0.97,
        &["bm25"],
    );
    cache_tests.signature =
        "fn test_handle_search_surfaces_query_cache_for_caching_queries()".into();
    cache_tests.code = "assert query cache search results".into();

    let mut stem_tests = make_named_result(
        "stem-tests",
        "test_bm25_search_recovers_cache_from_caching_query",
        "crates/contextro-tools/src/search.rs",
        0.91,
        &["bm25"],
    );
    stem_tests.signature = "fn test_bm25_search_recovers_cache_from_caching_query()".into();
    stem_tests.code = "caching query normalization test".into();

    let mut hf_path = make_named_result(
        "hf-path",
        "find_hf_cache_path",
        "crates/contextro-tools/src/download.rs",
        0.88,
        &["vector"],
    );
    hf_path.signature = "fn find_hf_cache_path() -> PathBuf".into();
    hf_path.code = "resolve huggingface cache path".into();

    let mut query_cache = make_named_result(
        "query-cache",
        "QueryCache",
        "crates/contextro-engines/src/cache.rs",
        0.58,
        &["bm25", "vector"],
    );
    query_cache.signature = "pub struct QueryCache".into();
    query_cache.code = "stores cached search responses with TTL eviction and invalidation".into();

    let mut cache_get = make_named_result(
        "cache-get",
        "QueryCache::get",
        "crates/contextro-engines/src/cache.rs",
        0.55,
        &["bm25"],
    );
    cache_get.signature = "pub fn get(&self, query: &str) -> Option<SearchResponse>".into();
    cache_get.code = "returns cached search responses when entries have not expired".into();

    let reranked = rerank_natural_language_results(
        "how does caching work",
        vec![cache_tests, stem_tests, hf_path, query_cache, cache_get],
    );

    assert_eq!(
        reranked[0].filepath,
        "crates/contextro-engines/src/cache.rs"
    );
    assert_eq!(reranked[0].symbol_name, "QueryCache");
    assert_eq!(
        reranked[1].filepath,
        "crates/contextro-engines/src/cache.rs"
    );
    assert!(reranked
        .iter()
        .take(2)
        .all(is_probable_engine_internal_search_result));
}
