//! Search tool implementation.

use std::collections::{HashMap, HashSet};

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
        "vector" => vector_search(query, limit, vector_index),
        "hybrid" => {
            let core_results = {
                let options = SearchOptions {
                    query: query.into(),
                    limit: limit.saturating_mul(2).min(100),
                    language: language.clone(),
                    mode: "hybrid".into(),
                };
                let fusion = ReciprocalRankFusion::default();
                execute_search(&options, bm25, graph, cache, &fusion).results
            };
            let vec_results = vector_search(query, limit.saturating_mul(2).min(100), vector_index);
            if vec_results.is_empty() {
                core_results
            } else {
                fuse_results(core_results, vec_results, limit)
            }
        }
        _ => {
            let options = SearchOptions {
                query: query.into(),
                limit,
                language,
                mode,
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
    })
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

fn is_symbol_lookup_query(query: &str) -> bool {
    let trimmed = query.trim();
    !trimmed.is_empty() && trimmed.split_whitespace().count() == 1
}

fn result_matches_symbol_query(
    query: &str,
    normalized_query: &str,
    result: &SearchResult,
) -> bool {
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
    lexical: Vec<SearchResult>,
    vector: Vec<SearchResult>,
    limit: usize,
) -> Vec<SearchResult> {
    let mut metadata: HashMap<String, SearchResult> = HashMap::new();
    let mut scores: HashMap<String, f64> = HashMap::new();
    let mut sources: HashMap<String, HashSet<String>> = HashMap::new();

    for (rank, result) in lexical.into_iter().enumerate() {
        accumulate_result(&mut metadata, &mut scores, &mut sources, result, rank, 0.70);
    }
    for (rank, result) in vector.into_iter().enumerate() {
        accumulate_result(&mut metadata, &mut scores, &mut sources, result, rank, 0.30);
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
            signature: String::new(),
            match_sources: sources.iter().map(|source| (*source).to_string()).collect(),
        }
    }

    #[test]
    fn test_fuse_results_preserves_score_spread() {
        let fused = fuse_results(
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
}
