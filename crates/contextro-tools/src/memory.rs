//! Memory tools: remember, recall, forget, knowledge.

use serde_json::{json, Value};

pub fn handle_remember(args: &Value) -> Value {
    let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
    if content.is_empty() {
        return json!({"error": "Missing required parameter: content"});
    }
    let memory_type = args.get("memory_type").and_then(|v| v.as_str()).unwrap_or("note");
    let tags: Vec<String> = args.get("tags")
        .and_then(|v| v.as_str())
        .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
        .unwrap_or_default();

    // TODO: embed and store in LanceDB
    let id = format!("mem_{:08x}", content.len() as u32 ^ 0xdeadbeef);

    json!({
        "stored": true,
        "id": id,
        "memory_type": memory_type,
        "tags": tags,
    })
}

pub fn handle_recall(args: &Value) -> Value {
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
    if query.is_empty() {
        return json!({"error": "Missing required parameter: query"});
    }
    let _limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5);

    // TODO: semantic search over memories
    json!({
        "query": query,
        "memories": [],
        "total": 0,
    })
}

pub fn handle_forget(args: &Value) -> Value {
    let id = args.get("memory_id").and_then(|v| v.as_str());
    let tags = args.get("tags").and_then(|v| v.as_str());

    if id.is_none() && tags.is_none() {
        return json!({"error": "Provide memory_id or tags to forget"});
    }

    // TODO: delete from store
    json!({"deleted": 0})
}

pub fn handle_knowledge(args: &Value) -> Value {
    let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
    match command {
        "show" => json!({"knowledge_bases": []}),
        "add" => {
            let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name.is_empty() {
                return json!({"error": "Missing required parameter: name"});
            }
            json!({"status": "indexed", "name": name})
        }
        "search" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            json!({"query": query, "results": [], "total": 0})
        }
        "remove" => json!({"removed": true}),
        _ => json!({"error": format!("Unknown knowledge command: {}", command)}),
    }
}
