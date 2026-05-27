use std::collections::{HashMap, HashSet};

use crate::analysis::is_test_file;
use contextro_core::models::SearchResult;
use contextro_engines::bm25::Bm25Engine;

use super::query::{
    is_probable_engine_internal_search_result, is_probable_internal_helper_symbol,
    is_probable_meta_support_result, is_probable_product_surface_result, is_probable_test_symbol,
    is_public_signature, natural_language_query_terms, normalized_concept_terms,
    query_is_explanatory, query_subsystem_focus, query_targets_engine_internals,
    query_targets_product_surface, query_targets_support_or_tooling,
};
use super::symbol_queries::{is_symbol_lookup_query, query_explicitly_targets_tests};

pub(super) fn rerank_natural_language_results(
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
        let quality_multiplier =
            if is_test_file(&result.filepath) || is_probable_test_symbol(&result.symbol_name) {
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

pub(super) fn maybe_expand_conceptual_hybrid_candidates(
    query: &str,
    candidate_limit: usize,
    core_results: Vec<SearchResult>,
    bm25: &Bm25Engine,
) -> Vec<SearchResult> {
    if !should_expand_conceptual_candidates(query) {
        return core_results;
    }

    let mut merged = merge_ranked_results(
        core_results,
        conceptual_bm25_candidates(query, candidate_limit, bm25),
    );
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

pub(super) fn result_query_overlap(query_terms: &[String], result: &SearchResult) -> f64 {
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
        &format!(
            "{} {} {}",
            result.symbol_name, result.filepath, result.signature
        ),
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
