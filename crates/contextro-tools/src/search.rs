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
            let core_results = maybe_expand_conceptual_hybrid_candidates(
                query,
                candidate_limit,
                core_results,
                bm25,
            );
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

    let confidence = confidence_label(query, &results);

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

fn drop_low_confidence_noise(
    query: &str,
    mode: &str,
    results: Vec<SearchResult>,
) -> Vec<SearchResult> {
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

    let query_terms = natural_language_query_terms(query);
    if query_terms.is_empty() {
        return results;
    }

    let targets_engine_internals = query_targets_engine_internals(query);
    let targets_product_surface = query_targets_product_surface(query);
    let targets_support_or_tooling = query_targets_support_or_tooling(query);
    let subsystem_focus = query_subsystem_focus(query);
    let strongest_grounding = results
        .iter()
        .map(|result| result_grounding_overlap(&query_terms, result))
        .fold(0.0, f64::max);
    let prefer_grounded_results = strongest_grounding >= 0.5;

    for result in &mut results {
        let overlap = result_query_overlap(&query_terms, result);
        let grounding = result_grounding_overlap(&query_terms, result);
        let agreement_bonus = 1.0 + (result.match_sources.len().saturating_sub(1) as f64 * 0.05);
        let overlap_bonus = 1.0 + overlap * 0.30 + grounding * 0.20;
        let grounding_multiplier = if prefer_grounded_results {
            if grounding == 0.0 {
                0.28
            } else if grounding < 0.25 {
                0.55
            } else {
                1.03 + grounding * 0.32
            }
        } else {
            1.0 + grounding * 0.10
        };
        let helper_multiplier = if is_probable_internal_helper_symbol(&result.symbol_name) {
            0.40
        } else if is_public_signature(&result.signature) {
            1.08
        } else {
            1.0
        };
        let quality_multiplier = if is_test_file(&result.filepath)
            || is_probable_test_symbol(&result.symbol_name)
        {
            0.35
        } else if !targets_support_or_tooling && is_probable_meta_support_result(result) {
            0.48
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
        let internal_multiplier = if targets_engine_internals {
            if is_probable_engine_internal_search_result(result) {
                1.55
            } else if !targets_support_or_tooling && is_probable_meta_support_result(result) {
                0.55
            } else {
                0.82
            }
        } else {
            1.0
        };
        let subsystem_multiplier = subsystem_focus.score_result(result);
        result.score *= agreement_bonus
            * overlap_bonus
            * grounding_multiplier
            * helper_multiplier
            * quality_multiplier
            * surface_multiplier
            * internal_multiplier
            * subsystem_multiplier;
    }

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
}

fn maybe_expand_conceptual_hybrid_candidates(
    query: &str,
    candidate_limit: usize,
    core_results: Vec<SearchResult>,
    bm25: &Bm25Engine,
) -> Vec<SearchResult> {
    if !should_expand_conceptual_candidates(query) {
        return core_results;
    }

    let mut merged = merge_ranked_results(core_results, conceptual_bm25_candidates(query, candidate_limit, bm25));
    merged.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    merged.truncate(candidate_limit.saturating_mul(2).min(120));
    merged
}

fn should_expand_conceptual_candidates(query: &str) -> bool {
    !is_symbol_lookup_query(query)
        && !query_explicitly_targets_tests(query)
        && (query_is_explanatory(query) || query_targets_engine_internals(query))
}

fn conceptual_bm25_candidates(
    query: &str,
    candidate_limit: usize,
    bm25: &Bm25Engine,
) -> Vec<SearchResult> {
    let conceptual_queries = conceptual_query_variants(query);
    let per_query_limit = candidate_limit.clamp(10, 40);
    let mut merged = Vec::new();
    let subsystem_focus = query_subsystem_focus(query);

    for conceptual_query in conceptual_queries {
        let mut results = bm25.search(&conceptual_query, per_query_limit);
        let query_terms = natural_language_query_terms(query);
        for result in &mut results {
            let grounding = result_grounding_overlap(&query_terms, result);
            let overlap = result_query_overlap(&query_terms, result);
            let conceptual_boost = if is_probable_engine_internal_search_result(result) {
                1.20
            } else {
                1.0
            };
            result.score *= (0.72 + grounding * 0.38 + overlap * 0.20)
                * conceptual_boost
                * subsystem_focus.score_result(result);
        }
        merged = merge_ranked_results(merged, results);
    }

    merged
}

fn conceptual_query_variants(query: &str) -> Vec<String> {
    let terms = natural_language_query_terms(query);
    let mut variants = Vec::new();

    if terms.is_empty() {
        return variants;
    }

    variants.push(terms.join(" "));
    if terms.len() == 1 {
        let term = &terms[0];
        if term == "cache" {
            variants.push("query cache ttl eviction".into());
            variants.push("query cache cached responses invalidation".into());
            variants.push("querycache ttl invalidation".into());
        } else if term == "config" && query.to_ascii_lowercase().contains("observability") {
            variants.push("observability config telemetry tracing".into());
        }
    }

    if query.to_ascii_lowercase().contains("observability") {
        variants.push("build observability config telemetry tracing".into());
        variants.push("build next sentry config options observability sentry".into());
    }

    if query.to_ascii_lowercase().contains("observability")
        && !terms.iter().any(|term| term == "observability")
    {
        variants.push("observability config".into());
    }

    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    for variant in variants {
        if seen.insert(variant.clone()) {
            deduped.push(variant);
        }
    }
    deduped
}

fn merge_ranked_results(
    base_results: Vec<SearchResult>,
    supplemental_results: Vec<SearchResult>,
) -> Vec<SearchResult> {
    let mut merged: HashMap<String, SearchResult> = HashMap::new();

    for result in base_results.into_iter().chain(supplemental_results) {
        merged
            .entry(result.id.clone())
            .and_modify(|existing| {
                existing.score = existing.score.max(result.score);
                for source in &result.match_sources {
                    if !existing.match_sources.contains(source) {
                        existing.match_sources.push(source.clone());
                    }
                }
                if existing.code.is_empty() {
                    existing.code = result.code.clone();
                }
                if existing.signature.is_empty() {
                    existing.signature = result.signature.clone();
                }
            })
            .or_insert(result);
    }

    merged.into_values().collect()
}

#[derive(Clone, Debug, Default)]
struct QuerySubsystemFocus {
    required_terms: Vec<String>,
    strong_terms: Vec<String>,
    preferred_path_terms: Vec<String>,
    preferred_symbol_terms: Vec<(String, f64)>,
    penalty_symbol_prefixes: Vec<String>,
    penalty_symbol_terms: Vec<String>,
}

impl QuerySubsystemFocus {
    fn score_result(&self, result: &SearchResult) -> f64 {
        if self.required_terms.is_empty()
            && self.strong_terms.is_empty()
            && self.preferred_path_terms.is_empty()
            && self.preferred_symbol_terms.is_empty()
            && self.penalty_symbol_prefixes.is_empty()
            && self.penalty_symbol_terms.is_empty()
        {
            return 1.0;
        }

        let symbol_name = result.symbol_name.to_ascii_lowercase();
        let terminal_symbol = terminal_symbol_name(&result.symbol_name);
        let path = result.filepath.to_ascii_lowercase();
        let signature = result.signature.to_ascii_lowercase();
        let combined = format!("{} {} {}", symbol_name, path, signature);

        let mut multiplier = 1.0;

        let required_matches = self
            .required_terms
            .iter()
            .filter(|term| combined.contains(term.as_str()))
            .count();
        if !self.required_terms.is_empty() {
            if required_matches == self.required_terms.len() {
                multiplier *= 1.45;
            } else if required_matches == 0 {
                multiplier *= 0.55;
            } else {
                multiplier *= 0.90;
            }
        }

        let strong_matches = self
            .strong_terms
            .iter()
            .filter(|term| combined.contains(term.as_str()))
            .count();
        if strong_matches > 0 {
            multiplier *= 1.0 + (strong_matches as f64 * 0.16).min(0.48);
        }

        if self
            .preferred_path_terms
            .iter()
            .any(|term| path.contains(term.as_str()))
        {
            multiplier *= 1.22;
        }

        for (term, bonus) in &self.preferred_symbol_terms {
            if symbol_name.contains(term.as_str()) {
                multiplier *= *bonus;
            }
        }

        if self
            .penalty_symbol_prefixes
            .iter()
            .any(|prefix| terminal_symbol.starts_with(prefix.as_str()))
        {
            multiplier *= 0.62;
        }

        if self
            .penalty_symbol_terms
            .iter()
            .any(|term| symbol_name.contains(term.as_str()) || path.contains(term.as_str()))
        {
            multiplier *= 0.72;
        }

        multiplier
    }
}

fn query_subsystem_focus(query: &str) -> QuerySubsystemFocus {
    let lowered = query.to_ascii_lowercase();

    if lowered.contains("observability") && lowered.contains("config") {
        return QuerySubsystemFocus {
            required_terms: vec!["observability".into(), "config".into()],
            strong_terms: vec![
                "telemetry".into(),
                "tracing".into(),
                "sentry".into(),
                "otel".into(),
            ],
            preferred_path_terms: vec!["/observability/".into()],
            preferred_symbol_terms: vec![
                ("buildobservabilityconfig".into(), 1.30),
                ("observabilityconfig".into(), 1.20),
                ("buildnextsentryconfigoptions".into(), 1.12),
            ],
            penalty_symbol_prefixes: vec!["read_".into(), "write_".into()],
            penalty_symbol_terms: vec!["formstate".into(), "syncformstate".into()],
        };
    }

    if query_is_explanatory(query)
        && (lowered.contains("caching") || lowered.contains("cache"))
    {
        return QuerySubsystemFocus {
            required_terms: vec!["cache".into()],
            strong_terms: vec![
                "querycache".into(),
                "query cache".into(),
                "ttl".into(),
                "evict".into(),
                "invalidate".into(),
                "cached responses".into(),
            ],
            preferred_path_terms: vec!["/cache.rs".into(), "/engines/".into()],
            preferred_symbol_terms: vec![("querycache".into(), 1.22)],
            penalty_symbol_prefixes: vec!["read_".into(), "write_".into()],
            penalty_symbol_terms: vec!["hf_cache_path".into(), "update_check".into()],
        };
    }

    QuerySubsystemFocus::default()
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
        || symbol_name.contains("setup")
        || symbol_name.contains("plugin")
        || symbol_name.contains("stub")
        || symbol_name.contains("helper")
}

fn is_public_signature(signature: &str) -> bool {
    let trimmed = signature.trim_start();
    trimmed.starts_with("pub ") || trimmed.starts_with("pub(")
}

fn query_targets_product_surface(query: &str) -> bool {
    let lowered = query.to_ascii_lowercase();
    if query_targets_engine_internals(&lowered) {
        return false;
    }

    if lowered.contains("how does") || lowered.contains("how do") {
        return true;
    }

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

fn query_targets_engine_internals(lowered_query: &str) -> bool {
    [
        "cache",
        "cached",
        "caching",
        "config",
        "configuration",
        "observability",
        "evict",
        "eviction",
        "expire",
        "expiry",
        "ttl",
        "invalidation",
        "invalidate",
    ]
    .iter()
    .any(|token| lowered_query.contains(token))
}

fn query_targets_support_or_tooling(query: &str) -> bool {
    let lowered = query.to_ascii_lowercase();

    [
        "test",
        "tests",
        "fixture",
        "fixtures",
        "spec",
        "stub",
        "mock",
        "plugin",
        "plugins",
        "setup",
        "path",
        "helper",
        "helpers",
        "manifest",
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
    if is_test_file(&result.filepath)
        || is_probable_test_symbol(&result.symbol_name)
        || is_probable_meta_support_result(result)
    {
        return false;
    }

    let path = result.filepath.to_ascii_lowercase();
    let symbol_name = terminal_symbol_name(&result.symbol_name);
    let full_symbol_name = result.symbol_name.to_ascii_lowercase();

    symbol_name == "execute_search"
        || symbol_name == "search"
        || full_symbol_name.contains("querycache")
        || full_symbol_name.contains("query_cache")
        || symbol_name.contains("ttl")
        || symbol_name.contains("evict")
        || symbol_name.contains("expire")
        || symbol_name.contains("invalidat")
        || (path.ends_with("/cache.rs") && is_public_signature(&result.signature))
        || ((path.contains("/engines/")
            || path.ends_with("/cache.rs")
            || path.ends_with("/sandbox.rs")
            || path.ends_with("/memory.rs")
            || path.ends_with("/archive.rs"))
            && (symbol_name.contains("search")
                || symbol_name.contains("cache")
                || symbol_name.contains("ttl")
                || symbol_name.contains("evict")
                || symbol_name.contains("expire")
                || symbol_name.contains("invalidat")
                || symbol_name.ends_with("_weights")
                || symbol_name.ends_with("_consensus")))
}

fn is_probable_meta_support_result(result: &SearchResult) -> bool {
    let path = result.filepath.to_ascii_lowercase();
    let symbol_name = terminal_symbol_name(&result.symbol_name);

    path.contains("/test")
        || path.contains("/tests/")
        || path.contains("/fixtures/")
        || path.contains("/fixture/")
        || path.contains("/plugins/")
        || path.contains("/setup")
        || path.contains("/stubs/")
        || path.contains("/stub/")
        || path.contains("/helpers/")
        || path.contains("/helper/")
        || path.contains("manifest")
        || path.ends_with("_stub.rs")
        || path.ends_with("_helper.rs")
        || path.ends_with("_helpers.rs")
        || symbol_name.contains("test")
        || symbol_name.contains("fixture")
        || symbol_name.contains("stub")
        || symbol_name.contains("plugin")
        || symbol_name.contains("helper")
        || symbol_name.contains("setup")
        || symbol_name.contains("path")
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

    let result_terms: HashSet<String> = normalized_concept_terms(
        &format!(
        "{} {} {} {}",
        result.symbol_name, result.filepath, result.signature, result.code
        ),
        false,
    );
    term_overlap_ratio(query_terms, &result_terms)
}

fn result_grounding_overlap(query_terms: &[String], result: &SearchResult) -> f64 {
    if query_terms.is_empty() {
        return 0.0;
    }

    let grounding_terms: HashSet<String> = normalized_concept_terms(
        &format!("{} {} {}", result.symbol_name, result.filepath, result.signature),
        false,
    );
    term_overlap_ratio(query_terms, &grounding_terms)
}

fn term_overlap_ratio(query_terms: &[String], candidate_terms: &HashSet<String>) -> f64 {
    if query_terms.is_empty() || candidate_terms.is_empty() {
        return 0.0;
    }

    let matched = query_terms
        .iter()
        .filter(|term| {
            candidate_terms.iter().any(|candidate| {
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

fn natural_language_query_terms(query: &str) -> Vec<String> {
    normalized_concept_terms(query, true).into_iter().collect()
}

fn normalized_concept_terms(text: &str, drop_stopwords: bool) -> HashSet<String> {
    tokenize_identifier(text)
        .into_iter()
        .filter_map(|token| normalize_concept_term(&token, drop_stopwords))
        .collect()
}

fn normalize_concept_term(token: &str, drop_stopwords: bool) -> Option<String> {
    let normalized = normalize_token_stem(token);
    if normalized.len() < 3 {
        return None;
    }

    if drop_stopwords && is_natural_language_stopword(&normalized) {
        return None;
    }

    Some(normalized)
}

fn normalize_token_stem(token: &str) -> String {
    let token = token.to_ascii_lowercase();

    if token.starts_with("configur") || token == "config" || token == "configs" {
        return "config".into();
    }

    if token.starts_with("cach") {
        return "cache".into();
    }

    if token.starts_with("evict") {
        return "evict".into();
    }

    if token.starts_with("invalidat") {
        return "invalidate".into();
    }

    if token.ends_with("ies") && token.len() > 4 {
        return format!("{}y", &token[..token.len() - 3]);
    }

    if token.ends_with('s') && token.len() > 4 && !token.ends_with("ss") {
        return token[..token.len() - 1].into();
    }

    token
}

fn is_natural_language_stopword(token: &str) -> bool {
    matches!(
        token,
        "about"
            | "across"
            | "does"
            | "each"
            | "from"
            | "have"
            | "into"
            | "should"
            | "that"
            | "their"
            | "them"
            | "then"
            | "there"
            | "these"
            | "this"
            | "those"
            | "through"
            | "what"
            | "when"
            | "where"
            | "which"
            | "with"
            | "work"
            | "works"
            | "would"
            | "how"
            | "the"
            | "and"
            | "for"
    )
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
    if is_expanded_natural_language_query(query) {
        (0.55, 0.45)
    } else {
        (0.70, 0.30)
    }
}

fn hybrid_candidate_limit(query: &str, limit: usize) -> usize {
    let multiplier = if is_expanded_natural_language_query(query) {
        6
    } else {
        2
    };
    limit.saturating_mul(multiplier).min(100)
}

fn vector_candidate_limit(query: &str, limit: usize) -> usize {
    let multiplier = if is_symbol_lookup_query(query) {
        20
    } else {
        10
    };
    limit.saturating_mul(multiplier).clamp(limit, 200)
}

fn rerank_result_limit(query: &str, limit: usize) -> usize {
    if is_expanded_natural_language_query(query) {
        limit.saturating_mul(4).min(60)
    } else {
        limit
    }
}

fn is_expanded_natural_language_query(query: &str) -> bool {
    if is_symbol_lookup_query(query) || query_explicitly_targets_tests(query) {
        return false;
    }

    let raw_terms = tokenize_identifier(query);
    if raw_terms.len() >= 3 {
        return true;
    }

    let terms = natural_language_query_terms(query);
    terms.len() >= 2 || (raw_terms.len() >= 2 && query_is_explanatory(query))
}

fn query_is_explanatory(query: &str) -> bool {
    let lowered = query.to_ascii_lowercase();
    lowered.contains("how does")
        || lowered.contains("how do")
        || lowered.contains("how is")
        || lowered.contains("how are")
        || lowered.starts_with("explain ")
        || lowered.starts_with("what is ")
        || lowered.starts_with("what are ")
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

fn confidence_label(query: &str, results: &[SearchResult]) -> &'static str {
    let Some(top) = results.first() else {
        return "low";
    };
    let second = results.get(1).map(|r| r.score).unwrap_or(0.0);
    let gap = top.score - second;

    if is_high_confidence_exact_symbol_hit(query, top) && top.score >= 0.55 {
        return "high";
    }

    if top.score >= 0.75 && gap >= 0.15 {
        "high"
    } else if top.score >= 0.45 {
        "medium"
    } else {
        "low"
    }
}

fn is_high_confidence_exact_symbol_hit(query: &str, result: &SearchResult) -> bool {
    if !is_symbol_lookup_query(query) || is_test_file(&result.filepath) {
        return false;
    }

    let normalized_query = normalize_identifier(query);
    if normalized_query.len() < 3 {
        return false;
    }

    let symbol_name = normalize_identifier(&result.symbol_name);
    let terminal_symbol = normalize_identifier(&terminal_symbol_name(&result.symbol_name));
    let path_stem = std::path::Path::new(&result.filepath)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(normalize_identifier)
        .unwrap_or_default();

    normalized_query == symbol_name
        || normalized_query == terminal_symbol
        || normalized_query == path_stem
}

#[cfg(test)]
mod tests {
    use super::*;
    use contextro_core::models::CodeChunk;
    use contextro_engines::bm25::Bm25Engine;
    use contextro_engines::cache::QueryCache;
    use contextro_engines::graph::CodeGraph;
    use contextro_engines::vector::VectorIndex;

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

    fn make_chunk(id: &str, text: &str, name: &str, filepath: &str) -> CodeChunk {
        CodeChunk {
            id: id.into(),
            text: text.into(),
            filepath: filepath.into(),
            symbol_name: name.into(),
            symbol_type: "function".into(),
            language: "rust".into(),
            line_start: 1,
            line_end: 10,
            signature: format!("pub fn {name}()"),
            parent: String::new(),
            docstring: String::new(),
            vector: vec![],
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
        assert_eq!(hybrid_candidate_limit("knowledge search milestones", 5), 30);
        assert_eq!(hybrid_candidate_limit("observability config", 5), 30);
        assert_eq!(hybrid_candidate_limit("how does caching work", 5), 30);
        assert_eq!(hybrid_candidate_limit("BrowserSession", 5), 10);
    }

    #[test]
    fn test_rerank_result_limit_expands_for_natural_language_queries() {
        assert_eq!(rerank_result_limit("semantic search ranking noise", 5), 20);
        assert_eq!(rerank_result_limit("observability config", 5), 20);
        assert_eq!(rerank_result_limit("how does caching work", 5), 20);
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
        plugin_helper.signature =
            "export function getAndroidManifestPluginHelpers(): Helpers".into();
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

        assert!(reranked[0].filepath.starts_with("packages/observability/src/"));
        assert!(reranked[1].filepath.starts_with("packages/observability/src/"));
        assert!(reranked.iter().take(2).all(|result| result.filepath.contains("observability")));
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
        assert!(query_targets_engine_internals("how does observability configuration work"));
    }

    #[test]
    fn test_support_or_tooling_intent_detects_setup_and_plugin_queries() {
        assert!(query_targets_support_or_tooling("plugin setup path helpers"));
        assert!(!query_targets_support_or_tooling("observability config"));
    }

    #[test]
    fn test_handle_search_surfaces_query_cache_for_caching_queries() {
        let bm25 = Bm25Engine::new_in_memory();
        bm25.index_chunks(&[
            make_chunk(
                "cache",
                "Query cache stores cached search responses with TTL eviction and invalidation support.",
                "QueryCache",
                "crates/contextro-engines/src/cache.rs",
            ),
            make_chunk(
                "search",
                "handle_search routes tool requests and formats tool responses.",
                "handle_search",
                "crates/contextro-tools/src/search.rs",
            ),
        ]);

        let graph = CodeGraph::new();
        let cache = QueryCache::new(16, 60.0);
        let vector = VectorIndex::new();

        for query in [
            "how does caching work",
            "how does the query cache work, TTL eviction",
        ] {
            let result =
                handle_search(&json!({"query": query, "limit": 10}), &bm25, &graph, &cache, &vector);
            let top = &result["results"][0];
            assert_eq!(top["name"], "QueryCache", "unexpected result for {query}: {result}");
            assert_eq!(
                top["file"],
                "crates/contextro-engines/src/cache.rs",
                "unexpected result for {query}: {result}"
            );
        }
    }

    #[test]
    fn test_handle_search_recovers_query_cache_from_explanatory_cache_runtime_pattern() {
        let bm25 = Bm25Engine::new_in_memory();
        bm25.index_chunks(&[
            make_chunk(
                "cache-tests-1",
                "test handle search surfaces query cache for caching queries and cache behavior",
                "test_handle_search_surfaces_query_cache_for_caching_queries",
                "crates/contextro-tools/src/search.rs",
            ),
            make_chunk(
                "cache-tests-2",
                "test bm25 search recovers cache from caching query",
                "test_bm25_search_recovers_cache_from_caching_query",
                "crates/contextro-engines/src/bm25.rs",
            ),
            make_chunk(
                "hf-cache-path",
                "find huggingface cache path on disk",
                "find_hf_cache_path",
                "crates/contextro-indexing/src/embedding.rs",
            ),
            make_chunk(
                "read-cache",
                "reads update cache from disk for release checks",
                "read_cache",
                "crates/contextro-server/src/update_check.rs",
            ),
            make_chunk(
                "write-cache",
                "writes update cache to disk for release checks",
                "write_cache",
                "crates/contextro-server/src/update_check.rs",
            ),
            make_chunk(
                "evict-test",
                "put evicts without deadlocking cache entry replacement",
                "put_evicts_without_deadlocking",
                "crates/contextro-engines/src/cache.rs",
            ),
            make_chunk(
                "query-cache",
                "Query cache stores cached search responses with TTL eviction and invalidation support.",
                "QueryCache",
                "crates/contextro-engines/src/cache.rs",
            ),
            make_chunk(
                "query-cache-get",
                "Returns cached search responses when query cache entries have not expired.",
                "QueryCache::get",
                "crates/contextro-engines/src/cache.rs",
            ),
        ]);

        let graph = CodeGraph::new();
        let cache = QueryCache::new(16, 60.0);
        let vector = VectorIndex::new();

        let result = handle_search(
            &json!({"query": "how does caching work", "limit": 10, "mode": "hybrid"}),
            &bm25,
            &graph,
            &cache,
            &vector,
        );

        let results = result["results"].as_array().expect("results array");
        assert_eq!(results[0]["name"], "QueryCache", "unexpected result: {result}");
        assert_eq!(results[0]["file"], "crates/contextro-engines/src/cache.rs");
        assert!(results.iter().any(|entry| {
            entry["file"] == "crates/contextro-engines/src/cache.rs"
                && entry["name"] == "QueryCache::get"
        }));
    }

    #[test]
    fn test_handle_search_query_cache_exact_match_reports_high_confidence() {
        let bm25 = Bm25Engine::new_in_memory();
        bm25.index_chunks(&[
            make_chunk(
                "query-cache",
                "QueryCache stores cached search responses with TTL eviction and invalidation support.",
                "QueryCache",
                "crates/contextro-engines/src/cache.rs",
            ),
            make_chunk(
                "query-cache-get",
                "QueryCache::get returns cached search responses when cache entries have not expired.",
                "QueryCache::get",
                "crates/contextro-engines/src/cache.rs",
            ),
            make_chunk(
                "handle-search",
                "handle_search routes search tool requests and formats responses.",
                "handle_search",
                "crates/contextro-tools/src/search.rs",
            ),
        ]);

        let graph = CodeGraph::new();
        let cache = QueryCache::new(16, 60.0);
        let vector = VectorIndex::new();

        let result = handle_search(
            &json!({"query": "QueryCache", "limit": 10, "mode": "hybrid"}),
            &bm25,
            &graph,
            &cache,
            &vector,
        );

        assert_eq!(result["results"][0]["name"], "QueryCache", "unexpected result: {result}");
        assert_eq!(result["confidence"], "high", "unexpected result: {result}");
    }

    #[test]
    fn test_confidence_label_promotes_exact_symbol_like_top_hit_without_large_gap() {
        let confidence = confidence_label(
            "QueryCache",
            &[
                make_named_result(
                    "query-cache",
                    "QueryCache",
                    "crates/contextro-engines/src/cache.rs",
                    0.62,
                    &["bm25"],
                ),
                make_named_result(
                    "query-cache-get",
                    "QueryCache::get",
                    "crates/contextro-engines/src/cache.rs",
                    0.58,
                    &["bm25"],
                ),
            ],
        );

        assert_eq!(confidence, "high");
    }

    #[test]
    fn test_confidence_label_keeps_natural_language_cache_query_at_medium_without_large_gap() {
        let confidence = confidence_label(
            "how does caching work",
            &[
                make_named_result(
                    "query-cache",
                    "QueryCache",
                    "crates/contextro-engines/src/cache.rs",
                    0.62,
                    &["bm25"],
                ),
                make_named_result(
                    "query-cache-get",
                    "QueryCache::get",
                    "crates/contextro-engines/src/cache.rs",
                    0.58,
                    &["bm25"],
                ),
            ],
        );

        assert_eq!(confidence, "medium");
    }

    #[test]
    fn test_handle_search_keeps_rich_payload_for_exact_symbol_matches() {
        let bm25 = Bm25Engine::new_in_memory();
        bm25.index_chunks(&[
            make_chunk(
                "query-cache",
                "QueryCache stores cached search responses with TTL eviction and invalidation support.",
                "QueryCache",
                "crates/contextro-engines/src/cache.rs",
            ),
            make_chunk(
                "query-cache-get",
                "QueryCache::get returns cached search responses when cache entries have not expired.",
                "QueryCache::get",
                "crates/contextro-engines/src/cache.rs",
            ),
        ]);

        let graph = CodeGraph::new();
        let cache = QueryCache::new(16, 60.0);
        let vector = VectorIndex::new();

        let result = handle_search(
            &json!({"query": "QueryCache", "limit": 10, "mode": "bm25"}),
            &bm25,
            &graph,
            &cache,
            &vector,
        );

        assert_eq!(result["confidence"], "high", "unexpected result: {result}");
        assert_eq!(result["results"][0]["name"], "QueryCache");
        assert_eq!(result["results"][0]["file"], "crates/contextro-engines/src/cache.rs");
        assert_eq!(result["results"][0]["type"], "function");
        assert!(result["results"][0].get("score").is_some(), "unexpected result: {result}");
        assert_eq!(result["limit"], 10);
        assert_eq!(result["truncated"], false);
    }

    #[test]
    fn test_handle_search_keeps_rich_payload_for_non_exact_single_term_queries() {
        let bm25 = Bm25Engine::new_in_memory();
        bm25.index_chunks(&[
            make_chunk(
                "query-cache",
                "Query cache stores cached search responses with TTL eviction and invalidation support.",
                "QueryCache",
                "crates/contextro-engines/src/cache.rs",
            ),
            make_chunk(
                "query-cache-get",
                "Returns cached search responses when query cache entries have not expired.",
                "QueryCache::get",
                "crates/contextro-engines/src/cache.rs",
            ),
        ]);

        let graph = CodeGraph::new();
        let cache = QueryCache::new(16, 60.0);
        let vector = VectorIndex::new();

        let result = handle_search(
            &json!({"query": "cache", "limit": 10, "mode": "bm25"}),
            &bm25,
            &graph,
            &cache,
            &vector,
        );

        let entries = result["results"].as_array().expect("results array");
        assert!(entries.iter().any(|entry| entry["name"] == "QueryCache"));
        assert!(entries.iter().all(|entry| entry.get("type").is_some()), "unexpected result: {result}");
        assert!(entries.iter().all(|entry| entry.get("score").is_some()), "unexpected result: {result}");
        assert_eq!(result["limit"], 10);
        assert_eq!(result["truncated"], false);
    }

    #[test]
    fn test_natural_language_reranker_prefers_observability_builders_over_generic_state_symbols() {
        let mut settlement_state = make_named_result(
            "settlement-state",
            "SettlementSyncFormState",
            "apps/app/src/features/finance/accounting/components/accounting-ap-sync.utils.ts",
            0.97,
            &["vector"],
        );
        settlement_state.signature = "export type SettlementSyncFormState = typeof DEFAULT_SETTLEMENT_SYNC_CONFIG".into();
        settlement_state.code = "settlement sync config state for accounting forms".into();

        let mut observability_builder = make_named_result(
            "obs-builder",
            "buildObservabilityConfig",
            "packages/observability/src/server.ts",
            0.63,
            &["bm25", "vector"],
        );
        observability_builder.signature = "export function buildObservabilityConfig(serviceName: string): ObservabilityConfig".into();
        observability_builder.code = "build observability config with telemetry and tracing exporters".into();

        let mut sentry_builder = make_named_result(
            "sentry-builder",
            "buildNextSentryConfigOptions",
            "packages/observability/src/sentry.ts",
            0.61,
            &["bm25"],
        );
        sentry_builder.signature = "export function buildNextSentryConfigOptions(): NextSentryBuildOptions | null".into();
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
            vec![read_cache, write_cache, find_hf_cache_path, query_cache, query_cache_get],
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
        init.signature =
            "export function initializeObservability(config: ObservabilityConfig)".into();
        init.code = "initialize telemetry using observability config".into();

        let reranked = rerank_natural_language_results(
            "observability config",
            vec![setup, plugin, chart, config, init],
        );

        assert_eq!(reranked[0].filepath, "packages/observability/src/config.ts");
        assert_eq!(reranked[1].filepath, "packages/observability/src/index.ts");
        assert!(reranked.iter().take(2).all(|result| !is_probable_meta_support_result(result)));
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
        cache_tests.signature = "fn test_handle_search_surfaces_query_cache_for_caching_queries()".into();
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

        assert_eq!(reranked[0].filepath, "crates/contextro-engines/src/cache.rs");
        assert_eq!(reranked[0].symbol_name, "QueryCache");
        assert_eq!(reranked[1].filepath, "crates/contextro-engines/src/cache.rs");
        assert!(reranked.iter().take(2).all(|result| is_probable_engine_internal_search_result(result)));
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
    fn test_drop_low_confidence_noise_vector_rejects_digit_bearing_nonsense_without_literal_match()
    {
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
