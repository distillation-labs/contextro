//! Search tool implementation.

use std::collections::{HashMap, HashSet};

use crate::analysis::is_test_file;
use contextro_core::models::SearchResult;
use contextro_engines::bm25::Bm25Engine;
use contextro_engines::cache::QueryCache;
use contextro_engines::fusion::ReciprocalRankFusion;
use contextro_engines::graph::CodeGraph;
use contextro_engines::search::{execute_search, SearchOptions};
use contextro_engines::vector::VectorIndex;
use contextro_indexing::embed;
use serde_json::{json, Value};

/// Execute the search tool.
pub fn handle_search(
    args: &Value,
    bm25: &Bm25Engine,
    graph: &CodeGraph,
    cache: &QueryCache,
    vector_index: &VectorIndex,
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

    let mut results = match mode.as_str() {
        "vector" => vector_search(query, vector_candidate_limit(query, limit), vector_index),
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
            let vec_results = vector_search(query, candidate_limit, vector_index);
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

    // #2: Import-aware search — boost results from files connected to context_files
    let context_files: Vec<&str> = match args.get("context_files") {
        Some(Value::Array(arr)) => arr.iter().filter_map(|v| v.as_str()).collect(),
        Some(Value::String(s)) => s.split(',').map(|s| s.trim()).collect(),
        _ => vec![],
    };
    if !context_files.is_empty() {
        for r in &mut results {
            // Boost results whose filepath shares a directory with any context file
            for cf in &context_files {
                if let Some(dir) = std::path::Path::new(cf).parent() {
                    if r.filepath.starts_with(&dir.to_string_lossy().to_string()) {
                        r.score *= 1.3; // 30% boost for same-directory results
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

    let confidence = confidence_label(&results);

    let out: Vec<Value> = results
        .iter()
        .map(|r| {
            json!({
                "name": r.symbol_name,
                "file": r.filepath,
                "line": r.line_start,
                "type": r.symbol_type,
                "score": (r.score * 10000.0).round() / 10000.0,
            })
        })
        .collect();

    json!({
        "query": query,
        "confidence": confidence,
        "results": out,
        "total": total,
        "limit": limit,
        "truncated": total > limit,
    })
}

fn drop_low_confidence_noise(query: &str, mode: &str, results: Vec<SearchResult>) -> Vec<SearchResult> {
    let mut results = results;

    if results.is_empty() {
        return results;
    }

    if mode == "vector" && vector_query_requires_literal_grounding(query) {
        results.retain(|result| result_has_literal_query_grounding(query, result));
        if results.is_empty() {
            return results;
        }
    }

    let min_score = if mode == "vector" {
        if is_symbol_lookup_query(query) {
            0.15
        } else {
            0.18
        }
    } else if is_symbol_lookup_query(query) {
        0.12
    } else {
        0.18
    };

    let relative_floor = if mode == "vector" {
        let top_score = results[0].score.max(0.0);
        if top_score >= min_score {
            let ratio = if is_symbol_lookup_query(query) {
                0.72
            } else {
                0.70
            };
            top_score * ratio
        } else {
            min_score
        }
    } else {
        min_score
    };

    results
        .into_iter()
        .filter(|result| result.score >= min_score && result.score >= relative_floor)
        .collect()
}

fn vector_query_requires_literal_grounding(query: &str) -> bool {
    let trimmed = query.trim();
    !trimmed.is_empty()
        && trimmed.split_whitespace().count() == 1
        && trimmed.chars().any(|ch| ch.is_ascii_digit())
}

fn result_has_literal_query_grounding(query: &str, result: &SearchResult) -> bool {
    let normalized_query = normalize_identifier(query);
    if normalized_query.len() < 3 {
        return true;
    }

    [
        result.symbol_name.as_str(),
        result.filepath.as_str(),
        result.signature.as_str(),
    ]
    .iter()
    .any(|field| normalize_identifier(field).contains(&normalized_query))
}

fn apply_symbol_query_guard(query: &str, results: Vec<SearchResult>) -> Vec<SearchResult> {
    if !is_symbol_lookup_query(query) {
        return results;
    }

    let normalized_query = normalize_identifier(query);
    if normalized_query.len() < 3 {
        return results;
    }

    results
        .into_iter()
        .filter(|result| result_matches_symbol_query(query, &normalized_query, result))
        .collect()
}

fn rerank_natural_language_results(
    query: &str,
    mut results: Vec<SearchResult>,
) -> Vec<SearchResult> {
    if query.split_whitespace().count() < 2
        || is_symbol_lookup_query(query)
        || query_explicitly_targets_tests(query)
    {
        return results;
    }

    if !results.iter().any(|result| !is_test_file(&result.filepath)) {
        return results;
    }

    let query_terms = tokenize_identifier(query);
    let targets_product_surface = query_targets_product_surface(query);
    for result in &mut results {
        let overlap = result_query_overlap(&query_terms, result);
        let agreement_bonus = 1.0 + (result.match_sources.len().saturating_sub(1) as f64 * 0.05);
        let overlap_bonus = 1.0 + overlap * 0.35;
        let helper_multiplier = if is_probable_internal_helper_symbol(&result.symbol_name) {
            0.40
        } else if is_public_signature(&result.signature) {
            1.08
        } else {
            1.0
        };
        let quality_multiplier =
            if is_test_file(&result.filepath) || is_probable_test_symbol(&result.symbol_name) {
                0.35
            } else {
                1.03
            };
        let surface_multiplier = if targets_product_surface {
            if is_probable_product_surface_result(result) {
                1.35
            } else if is_probable_engine_internal_search_result(result) {
                0.65
            } else {
                1.0
            }
        } else {
            1.0
        };
        result.score *= agreement_bonus
            * overlap_bonus
            * helper_multiplier
            * quality_multiplier
            * surface_multiplier;
    }

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
}

fn is_probable_test_symbol(symbol_name: &str) -> bool {
    let symbol_name = terminal_symbol_name(symbol_name);

    symbol_name == "tests"
        || symbol_name.starts_with("test_")
        || symbol_name.ends_with("_test")
        || symbol_name.starts_with("bench_")
}

fn is_probable_internal_helper_symbol(symbol_name: &str) -> bool {
    let symbol_name = terminal_symbol_name(symbol_name);

    symbol_name.starts_with("make_")
        || symbol_name.starts_with("normalize_")
        || symbol_name.starts_with("tokenize_")
        || symbol_name.starts_with("accumulate_")
        || symbol_name.starts_with("confidence_")
        || symbol_name.ends_with("_for_query")
        || symbol_name.ends_with("_query_overlap")
        || symbol_name.ends_with("_candidate_limit")
        || symbol_name.ends_with("_weights")
}

fn is_public_signature(signature: &str) -> bool {
    let trimmed = signature.trim_start();
    trimmed.starts_with("pub ") || trimmed.starts_with("pub(")
}

fn query_targets_product_surface(query: &str) -> bool {
    let lowered = query.to_ascii_lowercase();
    [
        "alias",
        "contract",
        "developer",
        "mcp",
        "noise",
        "output",
        "persistence",
        "persist",
        "ranking",
        "response",
        "surface",
        "tool",
        "workflow",
    ]
    .iter()
    .any(|token| lowered.contains(token))
}

fn is_probable_product_surface_result(result: &SearchResult) -> bool {
    let path = result.filepath.to_ascii_lowercase();
    let symbol_name = terminal_symbol_name(&result.symbol_name);

    symbol_name.starts_with("handle_")
        || path.contains("/tools/")
        || path.contains("/server/")
        || path.contains("/routes/")
        || path.contains("/handlers/")
        || path.contains("/commands/")
}

fn is_probable_engine_internal_search_result(result: &SearchResult) -> bool {
    let path = result.filepath.to_ascii_lowercase();
    let symbol_name = terminal_symbol_name(&result.symbol_name);

    symbol_name == "execute_search"
        || symbol_name == "search"
        || ((path.contains("/engines/")
            || path.ends_with("/memory.rs")
            || path.ends_with("/archive.rs"))
            && (symbol_name.contains("search")
                || symbol_name.ends_with("_weights")
                || symbol_name.ends_with("_consensus")))
}

fn terminal_symbol_name(symbol_name: &str) -> String {
    symbol_name
        .rsplit("::")
        .next()
        .unwrap_or(symbol_name)
        .rsplit('.')
        .next()
        .unwrap_or(symbol_name)
        .to_ascii_lowercase()
}

fn result_query_overlap(query_terms: &[String], result: &SearchResult) -> f64 {
    if query_terms.is_empty() {
        return 0.0;
    }

    let result_terms: HashSet<String> = tokenize_identifier(&format!(
        "{} {} {} {}",
        result.symbol_name, result.filepath, result.signature, result.code
    ))
    .into_iter()
    .collect();
    let matched = query_terms
        .iter()
        .filter(|term| {
            result_terms.iter().any(|candidate| {
                candidate == *term
                    || candidate.contains(term.as_str())
                    || term.contains(candidate.as_str())
            })
        })
        .count();
    matched as f64 / query_terms.len() as f64
}

fn is_symbol_lookup_query(query: &str) -> bool {
    let trimmed = query.trim();
    !trimmed.is_empty() && trimmed.split_whitespace().count() == 1
}

fn query_explicitly_targets_tests(query: &str) -> bool {
    let lowered = query.to_ascii_lowercase();
    ["test", "tests", "pytest", "spec", "fixture"]
        .iter()
        .any(|token| lowered.contains(token))
}

fn result_matches_symbol_query(query: &str, normalized_query: &str, result: &SearchResult) -> bool {
    let query_tokens = tokenize_identifier(query);
    if query_tokens.is_empty() {
        return true;
    }

    let matched = [result.symbol_name.as_str(), result.filepath.as_str()]
        .iter()
        .map(|field| {
            let normalized_field = normalize_identifier(field);
            if !normalized_query.is_empty() && normalized_field.contains(normalized_query) {
                return query_tokens.len();
            }

            let field_tokens: HashSet<String> = tokenize_identifier(field).into_iter().collect();
            query_tokens
                .iter()
                .filter(|token| {
                    field_tokens.iter().any(|candidate| {
                        candidate.contains(token.as_str()) || token.contains(candidate)
                    })
                })
                .count()
        })
        .max()
        .unwrap_or(0);

    match query_tokens.len() {
        0 => true,
        1 => matched == 1,
        2 => matched == 2,
        _ => matched * 2 >= query_tokens.len(),
    }
}

fn normalize_identifier(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

fn tokenize_identifier(text: &str) -> Vec<String> {
    let mut spaced = String::with_capacity(text.len() * 2);
    let mut prev_was_lower_or_digit = false;

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && prev_was_lower_or_digit {
                spaced.push(' ');
            }
            spaced.push(ch.to_ascii_lowercase());
            prev_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else {
            spaced.push(' ');
            prev_was_lower_or_digit = false;
        }
    }

    spaced
        .split_whitespace()
        .filter(|token| token.len() >= 3)
        .map(String::from)
        .collect()
}

fn vector_search(query: &str, limit: usize, index: &VectorIndex) -> Vec<SearchResult> {
    if index.is_empty() {
        return vec![];
    }
    match embed(query) {
        Some(qv) => index
            .search(&qv, limit)
            .into_iter()
            .filter(|result| result.score.is_finite() && result.score > 0.0)
            .collect(),
        None => vec![],
    }
}

/// Combine lexical/graph and vector signals without collapsing both tops to 1.0.
fn fuse_results(
    query: &str,
    lexical: Vec<SearchResult>,
    vector: Vec<SearchResult>,
    limit: usize,
) -> Vec<SearchResult> {
    let mut metadata: HashMap<String, SearchResult> = HashMap::new();
    let mut scores: HashMap<String, f64> = HashMap::new();
    let mut sources: HashMap<String, HashSet<String>> = HashMap::new();
    let (lexical_weight, vector_weight) = fusion_weights_for_query(query);

    for (rank, result) in lexical.into_iter().enumerate() {
        accumulate_result(
            &mut metadata,
            &mut scores,
            &mut sources,
            result,
            rank,
            lexical_weight,
        );
    }
    for (rank, result) in vector.into_iter().enumerate() {
        accumulate_result(
            &mut metadata,
            &mut scores,
            &mut sources,
            result,
            rank,
            vector_weight,
        );
    }

    let mut fused: Vec<SearchResult> = scores
        .into_iter()
        .filter_map(|(id, score)| {
            let mut result = metadata.remove(&id)?;
            let mut match_sources: Vec<String> = sources.remove(&id)?.into_iter().collect();
            match_sources.sort();
            result.score = score.min(1.0);
            result.match_sources = match_sources;
            Some(result)
        })
        .collect();

    fused.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    fused.truncate(limit);
    fused
}

fn fusion_weights_for_query(query: &str) -> (f64, f64) {
    if query.split_whitespace().count() >= 3
        && !is_symbol_lookup_query(query)
        && !query_explicitly_targets_tests(query)
    {
        (0.55, 0.45)
    } else {
        (0.70, 0.30)
    }
}

fn hybrid_candidate_limit(query: &str, limit: usize) -> usize {
    let multiplier = if query.split_whitespace().count() >= 3 && !is_symbol_lookup_query(query) {
        4
    } else {
        2
    };
    limit.saturating_mul(multiplier).min(100)
}

fn vector_candidate_limit(query: &str, limit: usize) -> usize {
    let multiplier = if is_symbol_lookup_query(query) { 20 } else { 10 };
    limit.saturating_mul(multiplier).clamp(limit, 200)
}

fn rerank_result_limit(query: &str, limit: usize) -> usize {
    if query.split_whitespace().count() >= 3 && !is_symbol_lookup_query(query) {
        limit.saturating_mul(3).min(50)
    } else {
        limit
    }
}

fn accumulate_result(
    metadata: &mut HashMap<String, SearchResult>,
    scores: &mut HashMap<String, f64>,
    sources: &mut HashMap<String, HashSet<String>>,
    result: SearchResult,
    rank: usize,
    engine_weight: f64,
) {
    let id = result.id.clone();
    let raw_score = result.score.clamp(0.0, 1.0);
    let rank_score = 1.0 / (rank as f64 + 1.0);
    let contribution = engine_weight * (raw_score * 0.85 + rank_score * 0.15);

    *scores.entry(id.clone()).or_default() += contribution;
    metadata.entry(id.clone()).or_insert_with(|| result.clone());
    let entry_sources = sources.entry(id).or_default();
    if result.match_sources.is_empty() {
        entry_sources.insert("unknown".into());
    } else {
        entry_sources.extend(result.match_sources.iter().cloned());
    }
}

fn confidence_label(results: &[SearchResult]) -> &'static str {
    let Some(top) = results.first() else {
        return "low";
    };
    let second = results.get(1).map(|r| r.score).unwrap_or(0.0);
    let gap = top.score - second;

    if top.score >= 0.75 && gap >= 0.15 {
        "high"
    } else if top.score >= 0.45 {
        "medium"
    } else {
        "low"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(id: &str, score: f64, sources: &[&str]) -> SearchResult {
        SearchResult {
            id: id.into(),
            filepath: format!("{id}.rs"),
            symbol_name: id.into(),
            symbol_type: "function".into(),
            language: "rust".into(),
            line_start: 1,
            line_end: 1,
            score,
            code: String::new(),
            signature: String::new(),
            match_sources: sources.iter().map(|source| (*source).to_string()).collect(),
        }
    }

    fn make_named_result(
        id: &str,
        symbol_name: &str,
        filepath: &str,
        score: f64,
        sources: &[&str],
    ) -> SearchResult {
        SearchResult {
            id: id.into(),
            filepath: filepath.into(),
            symbol_name: symbol_name.into(),
            symbol_type: "function".into(),
            language: "rust".into(),
            line_start: 1,
            line_end: 1,
            score,
            code: String::new(),
            signature: "pub fn sample()".into(),
            match_sources: sources.iter().map(|source| (*source).to_string()).collect(),
        }
    }

    #[test]
    fn test_fuse_results_preserves_score_spread() {
        let fused = fuse_results(
            "search ranking noise",
            vec![make_result("lexical_top", 1.0, &["bm25"])],
            vec![make_result("vector_top", 0.96, &["vector"])],
            10,
        );

        assert_eq!(fused.len(), 2);
        assert!(fused[0].score < 1.0);
        assert!(fused[1].score < fused[0].score);
    }

    #[test]
    fn test_fuse_results_rewards_cross_engine_agreement() {
        let fused = fuse_results(
            "search ranking noise",
            vec![
                make_result("shared", 0.92, &["bm25", "graph"]),
                make_result("lexical_only", 1.0, &["bm25"]),
            ],
            vec![make_result("shared", 0.88, &["vector"])],
            10,
        );

        assert_eq!(fused[0].id, "shared");
        assert!(fused[0].score > fused[1].score);
    }

    #[test]
    fn test_fusion_weights_favor_vector_for_natural_language_queries() {
        assert_eq!(
            fusion_weights_for_query("semantic search ranking noise"),
            (0.55, 0.45)
        );
        assert_eq!(fusion_weights_for_query("BrowserSession"), (0.70, 0.30));
    }

    #[test]
    fn test_hybrid_candidate_limit_expands_for_natural_language_queries() {
        assert_eq!(hybrid_candidate_limit("knowledge search milestones", 5), 20);
        assert_eq!(hybrid_candidate_limit("BrowserSession", 5), 10);
    }

    #[test]
    fn test_rerank_result_limit_expands_for_natural_language_queries() {
        assert_eq!(rerank_result_limit("semantic search ranking noise", 5), 15);
        assert_eq!(rerank_result_limit("BrowserSession", 5), 5);
    }

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

        let reranked =
            rerank_natural_language_results("hybrid search ranking", vec![engine, handler]);

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
        engine.signature =
            "pub fn execute_search(options: &SearchOptions) -> SearchResponse".into();
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
    fn test_drop_low_confidence_noise_removes_nonsense_hits() {
        let filtered = drop_low_confidence_noise(
            "xyznonexistent999",
            "bm25",
            vec![make_named_result(
                "noise",
                "test_knowledge_add_rejects_nonexistent_path_like_value",
                "crates/contextro-tools/src/memory.rs",
                0.0674,
                &["bm25"],
            )],
        );

        assert!(filtered.is_empty());
    }

    #[test]
    fn test_drop_low_confidence_noise_prunes_vector_tail_noise() {
        let filtered = drop_low_confidence_noise(
            "session archive persistence across restart",
            "vector",
            vec![
                make_named_result(
                    "top-hit",
                    "handle_retrieve",
                    "crates/contextro-tools/src/session.rs",
                    0.42,
                    &["vector"],
                ),
                make_named_result(
                    "tail-hit",
                    "random_helper",
                    "crates/contextro-tools/src/search.rs",
                    0.21,
                    &["vector"],
                ),
                make_named_result(
                    "noise-hit",
                    "test_search_fixture",
                    "crates/contextro-tools/src/search.rs",
                    0.12,
                    &["vector"],
                ),
            ],
        );

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].symbol_name, "handle_retrieve");
    }

    #[test]
    fn test_drop_low_confidence_noise_vector_rejects_digit_bearing_nonsense_without_literal_match() {
        let filtered = drop_low_confidence_noise(
            "xyznonexistent999",
            "vector",
            vec![
                make_named_result(
                    "noise-1",
                    "test_knowledge_add_rejects_nonexistent_path_like_value",
                    "crates/contextro-tools/src/memory.rs",
                    0.46,
                    &["vector"],
                ),
                make_named_result(
                    "noise-2",
                    "test_repo_add_reports_non_git_directory",
                    "crates/contextro-tools/src/git_tools.rs",
                    0.41,
                    &["vector"],
                ),
            ],
        );

        assert!(filtered.is_empty());
    }

    #[test]
    fn test_drop_low_confidence_noise_vector_keeps_digit_query_with_literal_grounding() {
        let filtered = drop_low_confidence_noise(
            "repo_add_v2",
            "vector",
            vec![make_named_result(
                "real-hit",
                "handle_repo_add_v2",
                "crates/contextro-tools/src/git_tools.rs",
                0.43,
                &["vector"],
            )],
        );

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].symbol_name, "handle_repo_add_v2");
    }
}
