use std::path::{Path, PathBuf};

use contextro_core::models::SearchResult;
use contextro_engines::graph::CodeGraph;
use serde_json::{json, Value};

use super::fusion::filter_results_by_language;
use super::symbol_queries::is_exact_symbol_lookup_query;

pub(super) fn resolved_search_codebase(
    codebase: Option<&str>,
    results: &[SearchResult],
) -> Option<String> {
    codebase
        .map(str::to_string)
        .or_else(|| infer_search_codebase(results))
}

fn infer_search_codebase(results: &[SearchResult]) -> Option<String> {
    let mut inferred_root: Option<PathBuf> = None;

    for result in results {
        let root = infer_repo_root(&result.filepath)?;
        match &inferred_root {
            Some(existing) if existing != &root => return None,
            Some(_) => {}
            None => inferred_root = Some(root),
        }
    }

    inferred_root.map(|root| root.to_string_lossy().to_string())
}

fn infer_repo_root(path: &str) -> Option<PathBuf> {
    let path = Path::new(path);
    if !path.is_absolute() {
        return None;
    }

    let mut current = path.parent();
    while let Some(dir) = current {
        if dir.join(".git").exists() {
            return Some(std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf()));
        }
        current = dir.parent();
    }

    None
}

pub(super) fn exact_symbol_graph_results(
    query: &str,
    limit: usize,
    language: Option<&str>,
    graph: &CodeGraph,
) -> Vec<SearchResult> {
    let mut results: Vec<SearchResult> = graph
        .find_nodes_by_name(query, true)
        .into_iter()
        .map(|node| {
            let (in_degree, out_degree) = graph.get_node_degree(&node.id);
            let connectivity_bonus = ((in_degree + out_degree) as f64).min(8.0) * 0.01;
            SearchResult {
                id: node.id,
                filepath: node.location.file_path,
                symbol_name: node.name,
                symbol_type: node.node_type.to_string(),
                language: node.language,
                line_start: node.location.start_line,
                line_end: node.location.end_line,
                score: (0.95 + connectivity_bonus).min(0.99),
                code: String::new(),
                signature: String::new(),
                match_sources: vec!["graph".into(), "exact".into()],
            }
        })
        .collect();
    results = filter_results_by_language(results, language);
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit);
    results
}

pub(super) fn search_tool_cache_key(
    query: &str,
    limit: usize,
    mode: &str,
    language: Option<&str>,
    context_files: &[String],
    codebase: Option<&str>,
) -> String {
    let mut normalized_context_files = context_files.to_vec();
    normalized_context_files.sort();
    serde_json::json!({
        "tool": "search",
        "query": query,
        "limit": limit,
        "mode": mode,
        "language": language.unwrap_or(""),
        "context_files": normalized_context_files,
        "codebase": codebase.unwrap_or(""),
    })
    .to_string()
}

pub(super) fn build_search_response(
    query: &str,
    limit: usize,
    total: usize,
    confidence: &str,
    results: &[SearchResult],
    codebase: Option<&str>,
    compact_exact_hit: bool,
) -> Value {
    let include_type =
        !(compact_exact_hit || (is_exact_symbol_lookup_query(query) && results.len() == 1));
    let out: Vec<Value> = results
        .iter()
        .map(|r| {
            let mut entry = json!({
                "name": r.symbol_name,
                "file": strip_codebase_path(&r.filepath, codebase),
                "line": r.line_start,
                "score": (r.score * 10000.0).round() / 10000.0,
            });
            if include_type {
                entry["type"] = json!(r.symbol_type);
            }
            entry
        })
        .collect();

    let mut response = json!({
        "query": query,
        "confidence": confidence,
        "results": out,
        "limit": limit,
    });
    if total > results.len() {
        response["total"] = json!(total);
        response["truncated"] = json!(true);
    }
    response
}

fn strip_codebase_path(path: &str, codebase: Option<&str>) -> String {
    if let Some(base) = codebase {
        let file_path = Path::new(path);
        if let Ok(relative) = file_path.strip_prefix(base) {
            return relative.to_string_lossy().to_string();
        }

        let canonical_file = std::fs::canonicalize(file_path).ok();
        let canonical_base = std::fs::canonicalize(base).ok();
        if let (Some(canonical_file), Some(canonical_base)) = (canonical_file, canonical_base) {
            if let Ok(relative) = canonical_file.strip_prefix(&canonical_base) {
                return relative.to_string_lossy().to_string();
            }
        }
    }

    path.to_string()
}
