//! Memory tools: remember, recall, forget, knowledge.

use std::collections::HashMap;

use chrono::Utc;
use parking_lot::RwLock;
use serde_json::{json, Value};

use contextro_core::models::{Memory, MemoryTtl, MemoryType};
use contextro_memory::store::MemoryStore;

pub fn handle_remember(args: &Value, store: &MemoryStore) -> Value {
    let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
    if content.is_empty() {
        return json!({"error": "Missing required parameter: content"});
    }
    let memory_type = parse_memory_type_arg(
        args.get("memory_type")
            .and_then(|v| v.as_str())
            .unwrap_or("note"),
    );
    let tags: Vec<String> = match args.get("tags") {
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        Some(Value::String(s)) => s
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        _ => vec![],
    };
    let ttl = parse_ttl_arg(
        args.get("ttl")
            .and_then(|v| v.as_str())
            .unwrap_or("permanent"),
    );
    let now = Utc::now().to_rfc3339();

    let memory = Memory {
        id: String::new(),
        content: content.into(),
        memory_type,
        project: args
            .get("project")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .into(),
        tags: tags.clone(),
        created_at: now.clone(),
        accessed_at: now,
        ttl,
        source: "user".into(),
    };

    match store.remember(&memory) {
        Ok(id) => {
            json!({"stored": true, "id": id, "memory_type": memory_type.to_string(), "tags": tags})
        }
        Err(e) => json!({"error": format!("Failed to store: {}", e)}),
    }
}

pub fn handle_recall(args: &Value, store: &MemoryStore) -> Value {
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
    if query.is_empty() {
        return json!({"error": "Missing required parameter: query"});
    }
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
    let memory_type = args.get("memory_type").and_then(|v| v.as_str());
    let tags = args.get("tags").and_then(|v| v.as_str());

    match store.recall(query, limit, memory_type, tags, None) {
        Ok(memories) => {
            let results: Vec<Value> = memories.iter().map(|m| {
                json!({"id": m.id, "content": m.content, "type": m.memory_type.to_string(), "tags": m.tags, "created_at": m.created_at})
            }).collect();
            json!({"query": query, "memories": results, "total": results.len()})
        }
        Err(e) => json!({"error": format!("Recall failed: {}", e)}),
    }
}

pub fn handle_forget(args: &Value, store: &MemoryStore) -> Value {
    let id = args.get("memory_id").and_then(|v| v.as_str());
    let tags_owned: Option<String> = match args.get("tags") {
        Some(Value::Array(arr)) => {
            let joined = arr
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(",");
            if joined.is_empty() { None } else { Some(joined) }
        }
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    };
    let tags = tags_owned.as_deref();
    let memory_type = args.get("memory_type").and_then(|v| v.as_str());

    if id.is_none() && tags.is_none() && memory_type.is_none() {
        return json!({"error": "Provide memory_id, tags, or memory_type to forget"});
    }

    match store.forget(id, tags, memory_type) {
        Ok(n) => json!({"deleted": n}),
        Err(e) => json!({"error": format!("Forget failed: {}", e)}),
    }
}

/// Knowledge base: simple in-memory doc store with substring search.
pub struct KnowledgeStore {
    docs: RwLock<HashMap<String, Vec<String>>>,
}

impl KnowledgeStore {
    pub fn new() -> Self {
        Self {
            docs: RwLock::new(HashMap::new()),
        }
    }

    /// Index content under `name`. Returns the number of chunks stored.
    pub fn add(&self, name: &str, content: &str) -> usize {
        if content.trim().is_empty() {
            return 0;
        }
        let lines: Vec<&str> = content.lines().collect();
        let chunks: Vec<String> = if lines.is_empty() {
            vec![]
        } else {
            lines.chunks(20).map(|c| c.join("\n")).collect()
        };
        let count = chunks.len();
        self.docs.write().insert(name.to_string(), chunks);
        count
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<(String, String)> {
        let query_lower = query.to_lowercase();
        let docs = self.docs.read();
        let mut results = Vec::new();
        for (name, chunks) in docs.iter() {
            for chunk in chunks {
                if chunk.to_lowercase().contains(&query_lower) {
                    results.push((name.clone(), chunk.clone()));
                    if results.len() >= limit {
                        return results;
                    }
                }
            }
        }
        results
    }

    pub fn show(&self) -> Vec<(String, usize)> {
        self.docs
            .read()
            .iter()
            .map(|(k, v)| (k.clone(), v.len()))
            .collect()
    }

    pub fn remove(&self, name: &str) -> bool {
        self.docs.write().remove(name).is_some()
    }
}

impl Default for KnowledgeStore {
    fn default() -> Self {
        Self::new()
    }
}

pub fn handle_knowledge(args: &Value, knowledge: &KnowledgeStore) -> Value {
    let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
    match command {
        "show" => {
            let bases: Vec<Value> = knowledge
                .show()
                .iter()
                .map(|(name, chunks)| json!({"name": name, "chunks": chunks}))
                .collect();
            json!({"knowledge_bases": bases, "total": bases.len()})
        }
        "add" => {
            let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let value = args.get("value").and_then(|v| v.as_str()).unwrap_or("");
            if name.is_empty() {
                return json!({"error": "Missing required parameter: name"});
            }
            // If value is a file/dir path, read it; otherwise treat as text
            let content = if std::path::Path::new(value).exists() {
                if std::path::Path::new(value).is_file() {
                    std::fs::read_to_string(value).unwrap_or_else(|_| value.to_string())
                } else {
                    // Directory: read all text files
                    let mut buf = String::new();
                    if let Ok(entries) = std::fs::read_dir(value) {
                        for entry in entries.flatten() {
                            if entry.path().is_file() {
                                if let Ok(text) = std::fs::read_to_string(entry.path()) {
                                    buf.push_str(&format!(
                                        "--- {} ---\n{}\n",
                                        entry.path().display(),
                                        text
                                    ));
                                }
                            }
                        }
                    }
                    buf
                }
            } else {
                value.to_string()
            };
            let chunk_count = knowledge.add(name, &content);
            if chunk_count == 0 {
                return json!({"error": "Content is empty — nothing indexed", "name": name});
            }
            json!({"status": "indexed", "name": name, "chunks": chunk_count})
        }
        "search" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            if query.is_empty() {
                return json!({"error": "Missing required parameter: query"});
            }
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
            let results: Vec<Value> = knowledge.search(query, limit).iter().map(|(name, chunk)| {
                json!({"source": name, "content": &chunk[..chunk.len().min(500)]})
            }).collect();
            json!({"query": query, "results": results, "total": results.len()})
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
            let content = if std::path::Path::new(path).is_file() {
                std::fs::read_to_string(path).unwrap_or_default()
            } else {
                String::new()
            };
            let chunk_count = knowledge.add(n, &content);
            json!({"status": "updated", "name": n, "chunks": chunk_count})
        }
        _ => json!({"error": format!("Unknown knowledge command: {}", command)}),
    }
}

fn parse_memory_type_arg(s: &str) -> MemoryType {
    match s {
        "conversation" => MemoryType::Conversation,
        "status" => MemoryType::Status,
        "decision" => MemoryType::Decision,
        "preference" => MemoryType::Preference,
        "doc" => MemoryType::Doc,
        _ => MemoryType::Note,
    }
}

fn parse_ttl_arg(s: &str) -> MemoryTtl {
    match s {
        "session" => MemoryTtl::Session,
        "day" => MemoryTtl::Day,
        "week" => MemoryTtl::Week,
        "month" => MemoryTtl::Month,
        _ => MemoryTtl::Permanent,
    }
}
