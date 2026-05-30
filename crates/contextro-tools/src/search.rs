//! Search tool implementation.

#[cfg(test)]
use contextro_core::models::SearchResult;
use contextro_engines::bm25::Bm25Engine;
use contextro_engines::cache::QueryCache;
use contextro_engines::fusion::ReciprocalRankFusion;
use contextro_engines::graph::CodeGraph;
use contextro_engines::search::{execute_search, SearchOptions};
use contextro_engines::vector::VectorIndex;
use serde_json::{json, Value};

mod fusion;
mod query;
mod ranking;
mod response;
mod symbol_queries;

#[cfg(test)]
use fusion::fusion_weights_for_query;
use fusion::{
    confidence_label, filter_results_by_language, fuse_results, hybrid_candidate_limit,
    rerank_result_limit, should_include_vector_signal_in_hybrid, vector_candidate_limit,
    vector_search,
};
#[cfg(test)]
use query::{
    is_probable_engine_internal_search_result, is_probable_meta_support_result,
    natural_language_query_terms, query_targets_engine_internals, query_targets_product_surface,
    query_targets_support_or_tooling,
};
use ranking::maybe_expand_conceptual_hybrid_candidates;
use ranking::rerank_natural_language_results;
#[cfg(test)]
use ranking::result_query_overlap;
use response::search_tool_cache_key;
use response::{build_search_response, exact_symbol_graph_results, resolved_search_codebase};
use symbol_queries::{
    apply_symbol_query_guard, drop_low_confidence_noise, is_bm25_identifier_exact_match_query,
    is_exact_symbol_lookup_query,
};

/// Execute the search tool.
pub fn handle_search(
    args: &Value,
    bm25: &Bm25Engine,
    graph: &CodeGraph,
    cache: &QueryCache,
    vector_index: &VectorIndex,
) -> Value {
    handle_search_with_codebase(args, bm25, graph, cache, vector_index, None)
}

pub fn handle_search_with_codebase(
    args: &Value,
    bm25: &Bm25Engine,
    graph: &CodeGraph,
    cache: &QueryCache,
    vector_index: &VectorIndex,
    codebase: Option<&str>,
) -> Value {
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
    if query.is_empty() {
        return json!({"error": "Missing required parameter: query"});
    }

    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let mode = args
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("hybrid")
        .to_string();
    let language = args
        .get("language")
        .and_then(|v| v.as_str())
        .map(String::from);
    let context_files: Vec<String> = match args.get("context_files") {
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        Some(Value::String(s)) => s
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(String::from)
            .collect(),
        _ => vec![],
    };
    let tool_cache_key = search_tool_cache_key(
        query,
        limit,
        &mode,
        language.as_deref(),
        &context_files,
        codebase,
    );
    if let Some(cached) = cache.get(&tool_cache_key) {
        return cached;
    }

    if let Some(exact_symbol_response) =
        exact_symbol_search_response(query, limit, &mode, language.as_deref(), graph, codebase)
    {
        cache.put(&tool_cache_key, exact_symbol_response.clone());
        return exact_symbol_response;
    }

    let mut results = match mode.as_str() {
        "vector" => {
            let vector_results =
                vector_search(query, vector_candidate_limit(query, limit), vector_index);
            filter_results_by_language(vector_results, language.as_deref())
        }
        "hybrid" => {
            let candidate_limit = hybrid_candidate_limit(query, limit);
            let rerank_limit = rerank_result_limit(query, limit);
            let core_results = {
                let options = SearchOptions {
                    query: query.into(),
                    limit: candidate_limit,
                    language: language.clone(),
                    mode: "hybrid".into(),
                };
                let fusion = ReciprocalRankFusion::default();
                execute_search(&options, bm25, graph, cache, &fusion).results
            };
            let core_results = maybe_expand_conceptual_hybrid_candidates(
                query,
                candidate_limit,
                core_results,
                bm25,
            );
            let vec_results = if should_include_vector_signal_in_hybrid(query) {
                filter_results_by_language(
                    vector_search(query, candidate_limit, vector_index),
                    language.as_deref(),
                )
            } else {
                vec![]
            };
            if vec_results.is_empty() {
                core_results
            } else {
                fuse_results(query, core_results, vec_results, rerank_limit)
            }
        }
        _ => {
            let options = SearchOptions {
                query: query.into(),
                limit,
                language,
                mode: mode.clone(),
            };
            let fusion = ReciprocalRankFusion::default();
            execute_search(&options, bm25, graph, cache, &fusion).results
        }
    };

    if !context_files.is_empty() {
        for r in &mut results {
            for cf in &context_files {
                if let Some(dir) = std::path::Path::new(cf).parent() {
                    if r.filepath.starts_with(&dir.to_string_lossy().to_string()) {
                        r.score *= 1.3;
                        break;
                    }
                }
            }
        }
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    results = apply_symbol_query_guard(query, results);
    results = rerank_natural_language_results(query, results);
    results = drop_low_confidence_noise(query, &mode, results);
    let total = results.len();
    results.truncate(limit);

    let confidence = confidence_label(query, &results);
    let response_codebase = resolved_search_codebase(codebase, &results);
    let response = build_search_response(
        query,
        limit,
        total,
        confidence,
        &results,
        response_codebase.as_deref(),
        false,
    );
    cache.put(&tool_cache_key, response.clone());
    response
}

fn exact_symbol_search_response(
    query: &str,
    limit: usize,
    mode: &str,
    language: Option<&str>,
    graph: &CodeGraph,
    codebase: Option<&str>,
) -> Option<Value> {
    if !matches!(mode, "hybrid" | "bm25") {
        return None;
    }
    let exact_symbol_like = is_exact_symbol_lookup_query(query);
    let unique_bm25_identifier_match =
        mode == "bm25" && !exact_symbol_like && is_bm25_identifier_exact_match_query(query);
    if !exact_symbol_like && !unique_bm25_identifier_match {
        return None;
    }

    let results = exact_symbol_graph_results(query, limit, language, graph);
    if results.is_empty() {
        return None;
    }
    if unique_bm25_identifier_match && results.len() != 1 {
        return None;
    }

    let confidence = confidence_label(query, &results);
    let response_codebase = resolved_search_codebase(codebase, &results);
    Some(build_search_response(
        query,
        limit,
        results.len(),
        confidence,
        &results,
        response_codebase.as_deref(),
        results.len() == 1,
    ))
}

#[cfg(test)]
mod tests;
