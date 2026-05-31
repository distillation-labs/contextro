//! Artifact tools: audit, docs_bundle, sidecar_export, skill_prompt, introspect, status, health, refactor_check, completion_check.

use crate::tool_manifest::{find_tool_doc, tool_docs, ToolDoc};
#[cfg(test)]
use contextro_engines::graph::CodeGraph;
use serde_json::{json, Value};

fn tool_doc_summary(doc: &ToolDoc) -> Value {
    json!({"name": doc.name, "tool": doc.name, "description": doc.description})
}

fn tool_doc_detail(doc: &ToolDoc) -> Value {
    json!({
        "name": doc.name,
        "tool": doc.name,
        "description": doc.description,
        "parameters": doc.parameters,
        "example": doc.example,
    })
}

fn tool_doc_haystack(doc: &ToolDoc) -> String {
    format!(
        "{} {} {} {}",
        doc.name,
        doc.description,
        doc.parameters.join(" "),
        doc.example
    )
    .to_lowercase()
}

mod audit;
mod export;

pub use audit::handle_audit;
#[cfg(test)]
pub(crate) use audit::{
    AUDIT_CONNECTION_THRESHOLD, AUDIT_EVIDENCE_LIMIT, AUDIT_FILE_SYMBOL_THRESHOLD,
};
pub use export::{handle_docs_bundle, handle_sidecar_export};

/// Print the agent bootstrap block.
pub fn handle_skill_prompt() -> Value {
    let core_tools = [
        "index",
        "search",
        "find_symbol",
        "find_callers",
        "find_callees",
        "explain",
        "impact",
        "code",
        "remember",
        "recall",
        "compact",
        "retrieve",
        "introspect",
    ]
    .iter()
    .filter_map(|name| find_tool_doc(name))
    .map(tool_doc_detail)
    .collect::<Vec<_>>();

    json!({
        "bootstrap": "# Contextro\n\nStart with `index({\"path\":\"/repo\"})`, then use `search` to find relevant code and `find_symbol` / `find_callers` / `find_callees` / `impact` to trace definitions and dependencies. Use `code` for AST-level symbol inspection, `remember` / `recall` for durable context, and `compact` + `retrieve` when a response is too large to keep inline.\n",
        "parameter_conventions": [
            "Prefer `symbol_name` for symbol tools; `name` and `symbol` aliases still work for backward compatibility.",
            "Use `path` for file and directory scoped tools such as `index`, `analyze`, `focus`, `docs_bundle`, and `sidecar_export`.",
            "Use `query` for search-style tools (`search`, `recall`, `commit_search`) and `tool` for exact `introspect` lookups.",
            "Use `compact` to create an archive ref, then pass the returned `ref_id` (currently `arc_...`) into `retrieve`.",
        ],
        "core_tools": core_tools,
    })
}

/// Look up Contextro's own tool docs.
pub fn handle_introspect(args: &Value) -> Value {
    let tool_filter = args
        .get("tool")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("");
    if !tool_filter.is_empty() {
        return match find_tool_doc(tool_filter) {
            Some(doc) => tool_doc_detail(doc),
            None => json!({"error": format!("Unknown tool: '{}'", tool_filter)}),
        };
    }

    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("");

    if query.is_empty() {
        let all: Vec<Value> = tool_docs().iter().map(tool_doc_summary).collect();
        return json!({"tools": all, "total": all.len()});
    }

    // Match tools where ANY query word appears in name or description.
    // Rank by number of matching words (more matches = more relevant).
    let words: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(|w| w.to_string())
        .collect();

    let mut scored: Vec<(usize, &ToolDoc)> = tool_docs()
        .iter()
        .filter_map(|doc| {
            let haystack = tool_doc_haystack(doc);
            let hits = words
                .iter()
                .filter(|w| haystack.contains(w.as_str()))
                .count();
            if hits > 0 {
                Some((hits, doc))
            } else {
                None
            }
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(b.1.name)));

    let matching: Vec<Value> = scored.iter().map(|(_, doc)| tool_doc_detail(doc)).collect();

    json!({"query": query, "matching_tools": matching, "total": matching.len()})
}

#[cfg(test)]
mod tests;
