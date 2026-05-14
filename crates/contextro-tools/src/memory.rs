//! Memory tools: remember, recall, forget, knowledge.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

pub fn handle_tags(store: &MemoryStore) -> Value {
    let tags = store.list_tags();
    json!({"tags": tags, "total": tags.len()})
}

pub fn handle_forget(args: &Value, store: &MemoryStore) -> Value {
    // Accept `memory_id` (current) or first element of `ids` array (v0.4.0 alias)
    let id_owned: Option<String> = args
        .get("memory_id")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| match args.get("ids") {
            Some(Value::Array(arr)) => arr.first().and_then(|v| v.as_str()).map(String::from),
            Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        });
    let id = id_owned.as_deref();
    let tags_owned: Option<String> = match args.get("tags") {
        Some(Value::Array(arr)) => {
            let joined = arr
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(",");
            if joined.is_empty() {
                None
            } else {
                Some(joined)
            }
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
        // Split into meaningful words (3+ chars) for fallback word matching
        let words: Vec<&str> = query_lower
            .split_whitespace()
            .filter(|w| w.len() >= 3)
            .collect();

        let docs = self.docs.read();
        let mut results: Vec<(String, String, usize)> = Vec::new();

        for (name, chunks) in docs.iter() {
            for chunk in chunks {
                let chunk_lower = chunk.to_lowercase();
                // Exact substring match scores highest
                if chunk_lower.contains(&query_lower) {
                    results.push((name.clone(), chunk.clone(), 100));
                } else if !words.is_empty() {
                    // Word overlap score
                    let matched = words.iter().filter(|w| chunk_lower.contains(*w)).count();
                    if matched > 0 {
                        results.push((name.clone(), chunk.clone(), matched));
                    }
                }
            }
        }

        results.sort_by_key(|r| std::cmp::Reverse(r.2));
        results
            .into_iter()
            .take(limit)
            .map(|(name, chunk, _)| (name, chunk))
            .collect()
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

fn read_knowledge_source(path: &Path) -> String {
    if path.is_file() {
        return std::fs::read_to_string(path).unwrap_or_default();
    }
    if !path.is_dir() {
        return String::new();
    }

    let mut files = collect_knowledge_files(path);
    files.sort();

    let mut content = String::new();
    for file in files {
        if let Ok(text) = std::fs::read_to_string(&file) {
            if text.trim().is_empty() {
                continue;
            }
            content.push_str(&format!("--- {} ---\n{}\n", file.display(), text));
        }
    }
    content
}

fn collect_knowledge_files(path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(path) else {
        return files;
    };

    for entry in entries.flatten() {
        let entry_path = entry.path();
        if entry_path.is_dir() {
            files.extend(collect_knowledge_files(&entry_path));
        } else if entry_path.is_file() {
            files.push(entry_path);
        }
    }

    files
}

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
            let content = if Path::new(value).exists() {
                read_knowledge_source(Path::new(value))
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
            let content = read_knowledge_source(Path::new(path));
            let chunk_count = knowledge.add(n, &content);
            if chunk_count == 0 {
                return json!({"error": "Content is empty — nothing indexed", "name": n});
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("contextro-knowledge-{unique}-{name}"))
    }

    #[test]
    fn test_knowledge_list_alias_returns_indexed_bases() {
        let knowledge = KnowledgeStore::new();
        assert_eq!(knowledge.add("docs", "chunk one\nchunk two"), 1);

        let result = handle_knowledge(&json!({"command":"list"}), &knowledge);
        assert_eq!(result["total"], 1);
        assert_eq!(result["knowledge_bases"][0]["name"], "docs");
        assert_eq!(result["knowledge_bases"][0]["chunks"], 1);
    }

    #[test]
    fn test_knowledge_add_indexes_nested_directory_contents() {
        let root = temp_dir("nested");
        let nested = root.join("docs/guides");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            nested.join("manual.md"),
            "Nested manual token: unique_nested_knowledge_token",
        )
        .unwrap();

        let knowledge = KnowledgeStore::new();
        let add_result = handle_knowledge(
            &json!({"command":"add","name":"nested-docs","value": root.to_string_lossy()}),
            &knowledge,
        );
        assert_eq!(add_result["status"], "indexed");

        let search_result = handle_knowledge(
            &json!({"command":"search","query":"unique_nested_knowledge_token","limit":5}),
            &knowledge,
        );
        assert_eq!(search_result["total"], 1);
        assert_eq!(search_result["results"][0]["source"], "nested-docs");

        let _ = std::fs::remove_dir_all(root);
    }
}
