use std::path::Path;

use serde_json::{json, Value};

use super::knowledge_state::{canonicalize_if_exists, read_knowledge_source, truncate_text};
use super::knowledge_store::KnowledgeStore;

pub fn handle_knowledge(args: &Value, knowledge: &KnowledgeStore) -> Value {
    // If `query` is provided without `command`, default to search (backward compat)
    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            if args.get("query").and_then(|v| v.as_str()).is_some() {
                "search"
            } else {
                ""
            }
        });
    match command {
        "show" | "list" => {
            let summaries = knowledge.show();
            let bases: Vec<Value> = match command {
                "show" => summaries
                    .iter()
                    .map(|summary| {
                        json!({
                            "name": summary.name,
                            "chunks": summary.chunks,
                            "preview": summary.preview,
                            "source_path": summary.source_path,
                        })
                    })
                    .collect(),
                _ => summaries
                    .iter()
                    .map(|summary| json!({"name": summary.name, "chunks": summary.chunks}))
                    .collect(),
            };
            json!({"knowledge_bases": bases, "total": bases.len()})
        }
        "add" => {
            let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let value = args.get("value").and_then(|v| v.as_str()).unwrap_or("");
            if name.is_empty() {
                return json!({"error": "Missing required parameter: name"});
            }
            if value.trim().is_empty() {
                return json!({"error": "Missing required parameter: value"});
            }
            let source_path = canonicalize_if_exists(Path::new(value));
            let content = source_path
                .as_deref()
                .map(read_knowledge_source)
                .unwrap_or_else(|| value.to_string());
            let overwritten = knowledge.contains(name);
            let chunk_count = knowledge.add(name, &content, source_path.as_deref());
            if chunk_count == 0 {
                return json!({"error": "Content is empty — nothing indexed", "name": name});
            }
            json!({
                "status": "indexed",
                "name": name,
                "chunks": chunk_count,
                "overwritten": overwritten,
            })
        }
        "search" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            if query.is_empty() {
                return json!({"error": "Missing required parameter: query"});
            }
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
            let results: Vec<Value> = knowledge
                .search(query, limit)
                .iter()
                .map(|(name, chunk)| json!({"source": name, "content": truncate_text(chunk, 500)}))
                .collect();
            json!({"query": query, "results": results, "total": results.len()})
        }
        "clear" => {
            let removed = knowledge.clear();
            json!({"status": "cleared", "removed": removed})
        }
        "remove" => {
            let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name.is_empty() {
                return json!({"error": "Missing required parameter: name"});
            }
            let removed = knowledge.remove(name);
            json!({"removed": removed, "name": name})
        }
        "update" => {
            let name = args.get("name").and_then(|v| v.as_str());
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if path.is_empty() {
                return json!({"error": "Missing required parameter: path"});
            }
            let n = name.unwrap_or(path);
            let source_path = canonicalize_if_exists(Path::new(path));
            if source_path.is_none() {
                return json!({"error": format!("Path not found: {}", path)});
            }
            let content = source_path
                .as_deref()
                .map(read_knowledge_source)
                .unwrap_or_default();
            let overwritten = knowledge.contains(n);
            let chunk_count = knowledge.add(n, &content, source_path.as_deref());
            if chunk_count == 0 {
                return json!({"error": "Content is empty — nothing indexed", "name": n});
            }
            json!({
                "status": "updated",
                "name": n,
                "chunks": chunk_count,
                "overwritten": overwritten,
            })
        }
        _ => json!({"error": format!("Unknown knowledge command: {}", command)}),
    }
}
