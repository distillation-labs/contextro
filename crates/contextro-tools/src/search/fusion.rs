use std::collections::{HashMap, HashSet};

use crate::analysis::is_test_file;
use contextro_core::models::SearchResult;
use contextro_engines::search::{classify_query, QueryType};
use contextro_engines::vector::VectorIndex;
use contextro_indexing::embed;

use super::query::{natural_language_query_terms, query_is_explanatory, terminal_symbol_name};
use super::symbol_queries::{
    is_symbol_lookup_query, normalize_identifier, query_explicitly_targets_tests,
    tokenize_identifier,
};

pub(super) fn vector_search(query: &str, limit: usize, index: &VectorIndex) -> Vec<SearchResult> {
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

pub(super) fn filter_results_by_language(
    mut results: Vec<SearchResult>,
    language: Option<&str>,
) -> Vec<SearchResult> {
    let Some(language) = language
        .map(str::trim)
        .filter(|language| !language.is_empty())
    else {
        return results;
    };

    results.retain(|result| result.language.eq_ignore_ascii_case(language));
    results
}

/// Combine lexical/graph and vector signals without collapsing both tops to 1.0.
pub(super) fn fuse_results(
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

pub(super) fn fusion_weights_for_query(query: &str) -> (f64, f64) {
    if is_expanded_natural_language_query(query) {
        (0.55, 0.45)
    } else {
        (0.70, 0.30)
    }
}

pub(super) fn hybrid_candidate_limit(query: &str, limit: usize) -> usize {
    let multiplier = if is_expanded_natural_language_query(query) {
        6
    } else {
        2
    };
    limit.saturating_mul(multiplier).min(100)
}

pub(super) fn vector_candidate_limit(query: &str, limit: usize) -> usize {
    let multiplier = if is_symbol_lookup_query(query) {
        20
    } else {
        10
    };
    limit.saturating_mul(multiplier).clamp(limit, 200)
}

pub(super) fn rerank_result_limit(query: &str, limit: usize) -> usize {
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

pub(super) fn should_include_vector_signal_in_hybrid(query: &str) -> bool {
    classify_query(query) != QueryType::Symbol
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

pub(super) fn confidence_label(query: &str, results: &[SearchResult]) -> &'static str {
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
