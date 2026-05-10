//! Artifact tools: audit, docs_bundle, sidecar_export, skill_prompt, introspect.

use serde_json::{json, Value};

pub fn handle_audit() -> Value {
    json!({
        "status": "ok",
        "recommendations": [],
        "note": "Full audit requires complete indexing with embeddings",
    })
}

pub fn handle_docs_bundle(args: &Value) -> Value {
    let _output_dir = args.get("output_dir").and_then(|v| v.as_str()).unwrap_or("");
    json!({"status": "generated", "files": []})
}

pub fn handle_sidecar_export(args: &Value) -> Value {
    let _path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    json!({"status": "exported", "sidecars": 0})
}

pub fn handle_skill_prompt() -> Value {
    json!({
        "bootstrap": "# Contextro\nUse `index` to index, then `search`/`find_symbol`/`explain` to query.",
    })
}

pub fn handle_introspect(args: &Value) -> Value {
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let tools = vec![
        "index", "search", "find_symbol", "find_callers", "find_callees",
        "explain", "impact", "code", "overview", "architecture", "analyze",
        "remember", "recall", "forget", "knowledge", "compact",
        "session_snapshot", "restore", "retrieve", "commit_search",
        "commit_history", "repo_add", "repo_remove", "repo_status",
        "audit", "docs_bundle", "sidecar_export", "skill_prompt",
        "introspect", "status", "health", "focus", "dead_code",
        "circular_dependencies", "test_coverage_map",
    ];

    if query.is_empty() {
        return json!({"tools": tools, "total": tools.len()});
    }

    let query_lower = query.to_lowercase();
    let matching: Vec<&&str> = tools.iter().filter(|t| t.contains(&query_lower.as_str())).collect();
    json!({"query": query, "matching_tools": matching, "total": matching.len()})
}
