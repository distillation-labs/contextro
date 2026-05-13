//! Search tool implementation.

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
            let bm25_results = {
                let options = SearchOptions {
                    query: query.into(),
                    limit,
                    language: language.clone(),
                    mode: "bm25".into(),
                };
                let fusion = ReciprocalRankFusion::default();
                execute_search(&options, bm25, graph, cache, &fusion).results
            };
            let vec_results = vector_search(query, limit, vector_index);
            if vec_results.is_empty() {
                bm25_results
            } else {
                fuse_results(bm25_results, vec_results, limit)
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

    let confidence = if results.is_empty() {
        0.0_f64
    } else {
        results[0].score.min(1.0)
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

    let out: Vec<Value> = results
        .iter()
        .map(|r| {
            json!({
                "name": r.symbol_name,
                "file": r.filepath,
                "line": r.line_start,
                "type": r.symbol_type,
                "score": (r.score * 1000.0).round() / 1000.0,
                "match": r.match_sources.join("+"),
            })
        })
        .collect();

    json!({
        "query": query,
        "confidence": (confidence * 1000.0).round() / 1000.0,
        "total": out.len(),
        "results": out,
    })
}

fn vector_search(query: &str, limit: usize, index: &VectorIndex) -> Vec<SearchResult> {
    if index.is_empty() {
        return vec![];
    }
    match embed(query) {
        Some(qv) => {
            let mut results = index.search(&qv, limit);
            // Normalize scores relative to the top result so the best match
            // shows confidence=1.0, making vector and BM25 scores comparable.
            if let Some(max) = results.first().map(|r| r.score) {
                if max > 0.0 {
                    for r in &mut results {
                        r.score /= max;
                    }
                }
            }
            results
        }
        None => vec![],
    }
}

/// Interleave BM25 and vector results, dedup by symbol name, keep highest score.
fn fuse_results(
    bm25: Vec<SearchResult>,
    vector: Vec<SearchResult>,
    limit: usize,
) -> Vec<SearchResult> {
    let mut seen = std::collections::HashSet::new();
    let mut fused: Vec<SearchResult> = Vec::new();

    // Normalise scores so both lists are in [0,1]
    let bm25_max = bm25.first().map(|r| r.score).unwrap_or(1.0).max(1e-9);
    let vec_max = vector.first().map(|r| r.score).unwrap_or(1.0).max(1e-9);

    let mut candidates: Vec<SearchResult> = bm25
        .into_iter()
        .map(|mut r| {
            r.score /= bm25_max;
            r.match_sources = vec!["bm25".into()];
            r
        })
        .chain(vector.into_iter().map(|mut r| {
            r.score /= vec_max;
            r.match_sources = vec!["vector".into()];
            r
        }))
        .collect();

    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for r in candidates {
        let key = format!("{}:{}", r.filepath, r.line_start);
        if seen.insert(key) {
            fused.push(r);
            if fused.len() >= limit {
                break;
            }
        }
    }
    fused
}
