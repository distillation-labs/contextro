//! Search tool implementation.

use serde_json::{json, Value};

use contextro_engines::bm25::Bm25Engine;
use contextro_engines::cache::QueryCache;
use contextro_engines::fusion::ReciprocalRankFusion;
use contextro_engines::graph::CodeGraph;
use contextro_engines::search::{execute_search, SearchOptions};

/// Execute the search tool.
pub fn handle_search(args: &Value, bm25: &Bm25Engine, graph: &CodeGraph, cache: &QueryCache) -> Value {
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
    if query.is_empty() {
        return json!({"error": "Missing required parameter: query"});
    }

    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("hybrid").to_string();
    let language = args.get("language").and_then(|v| v.as_str()).map(String::from);

    let options = SearchOptions { query: query.into(), limit, language, mode };
    let fusion = ReciprocalRankFusion::default();
    let response = execute_search(&options, bm25, graph, cache, &fusion);

    let results: Vec<Value> = response.results.iter().map(|r| {
        json!({
            "name": r.symbol_name,
            "file": r.filepath,
            "line": r.line_start,
            "type": r.symbol_type,
            "score": (r.score * 1000.0).round() / 1000.0,
            "match": r.match_sources.join("+"),
        })
    }).collect();

    json!({
        "query": response.query,
        "confidence": response.confidence,
        "total": response.total,
        "results": results,
    })
}
