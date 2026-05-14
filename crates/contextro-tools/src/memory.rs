//! Memory tools: remember, recall, forget, knowledge.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use contextro_config::get_settings;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
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
    // Accept `id` from remember(), `memory_id` (current), or first element of `ids` (v0.4.0 alias)
    let id_owned: Option<String> = args
        .get("id")
        .or_else(|| args.get("memory_id"))
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
        return json!({"error": "Provide id, memory_id, tags, or memory_type to forget"});
    }

    match store.forget(id, tags, memory_type) {
        Ok(n) => json!({"deleted": n}),
        Err(e) => json!({"error": format!("Forget failed: {}", e)}),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct KnowledgeChunk {
    content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct KnowledgeDocument {
    chunks: Vec<KnowledgeChunk>,
    metadata_text: String,
    source_path: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct PersistedKnowledgeState {
    #[serde(default = "default_knowledge_scope")]
    active_scope: String,
    #[serde(default)]
    scopes: HashMap<String, HashMap<String, KnowledgeDocument>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnowledgeDocSummary {
    pub name: String,
    pub chunks: usize,
    pub preview: Option<String>,
    pub source_path: Option<String>,
}

/// Knowledge base: lightweight in-memory doc store with metadata-aware search.
pub struct KnowledgeStore {
    state: RwLock<PersistedKnowledgeState>,
    file_path: PathBuf,
}

impl KnowledgeStore {
    pub fn new() -> Self {
        let storage_dir = get_settings().read().storage_dir.clone();
        Self::with_path(PathBuf::from(storage_dir).join("knowledge-store.json"))
    }

    pub fn with_path<P: Into<PathBuf>>(file_path: P) -> Self {
        let file_path = file_path.into();
        Self {
            state: RwLock::new(load_knowledge_state(&file_path)),
            file_path,
        }
    }

    pub fn set_active_scope(&self, scope: Option<&str>) {
        let scope = normalize_knowledge_scope(scope);
        let mut state = self.state.write();
        if state.active_scope == scope {
            return;
        }
        state.active_scope = scope;
        self.save_locked(&state);
    }

    /// Index content under `name`. Returns the number of chunks stored.
    pub fn add(&self, name: &str, content: &str, source_path: Option<&Path>) -> usize {
        if content.trim().is_empty() {
            return 0;
        }
        let lines: Vec<&str> = content.lines().collect();
        let chunks: Vec<KnowledgeChunk> = if lines.is_empty() {
            vec![]
        } else {
            lines
                .chunks(20)
                .map(|chunk_lines| KnowledgeChunk {
                    content: chunk_lines.join("\n"),
                })
                .collect()
        };
        let count = chunks.len();
        let mut state = self.state.write();
        let docs = active_docs_mut(&mut state);
        docs.insert(
            name.to_string(),
            KnowledgeDocument {
                chunks,
                metadata_text: knowledge_metadata_text(name, source_path),
                source_path: source_path.map(|path| path.to_string_lossy().to_string()),
            },
        );
        self.save_locked(&state);
        count
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<(String, String)> {
        let query_lower = query.to_lowercase();
        // Split into meaningful words (3+ chars) for fallback word matching
        let words: Vec<&str> = query_lower
            .split_whitespace()
            .filter(|w| w.len() >= 3)
            .collect();

        let state = self.state.read();
        let mut results: Vec<(String, String, usize)> = Vec::new();

        let Some(docs) = active_docs(&state) else {
            return Vec::new();
        };

        for (name, doc) in docs {
            let metadata_lower = doc.metadata_text.to_lowercase();
            for chunk in &doc.chunks {
                let chunk_lower = chunk.content.to_lowercase();

                // Search metadata (name/path aliases) and content together, but only
                // return the original chunk content so results stay truthful.
                let mut score = 0usize;

                if metadata_lower.contains(&query_lower) {
                    score += 120;
                }
                if chunk_lower.contains(&query_lower) {
                    score += 100;
                }
                if score == 0 && !words.is_empty() {
                    let metadata_matches = words
                        .iter()
                        .filter(|word| metadata_lower.contains(*word))
                        .count();
                    let content_matches = words
                        .iter()
                        .filter(|word| chunk_lower.contains(*word))
                        .count();
                    score = metadata_matches * 12 + content_matches * 4;
                }

                if score > 0 {
                    results.push((name.clone(), chunk.content.clone(), score));
                }
            }
        }

        results.sort_by_key(|result| Reverse(result.2));
        results
            .into_iter()
            .take(limit)
            .map(|(name, chunk, _)| (name, chunk))
            .collect()
    }

    pub fn show(&self) -> Vec<KnowledgeDocSummary> {
        let state = self.state.read();
        let Some(docs) = active_docs(&state) else {
            return Vec::new();
        };

        let mut summaries: Vec<KnowledgeDocSummary> = docs
            .iter()
            .map(|(name, doc)| KnowledgeDocSummary {
                name: name.clone(),
                chunks: doc.chunks.len(),
                preview: doc
                    .chunks
                    .first()
                    .and_then(|chunk| summarize_preview(&chunk.content, 120)),
                source_path: doc.source_path.clone(),
            })
            .collect();
        summaries.sort_by(|left, right| left.name.cmp(&right.name));
        summaries
    }

    pub fn remove(&self, name: &str) -> bool {
        let mut state = self.state.write();
        let scope = state.active_scope.clone();
        let mut removed = false;
        let mut scope_empty = false;
        if let Some(docs) = state.scopes.get_mut(&scope) {
            removed = docs.remove(name).is_some();
            scope_empty = docs.is_empty();
        }
        if removed {
            if scope_empty {
                state.scopes.remove(&scope);
            }
            self.save_locked(&state);
        }
        removed
    }

    fn save_locked(&self, state: &PersistedKnowledgeState) {
        if let Some(parent) = self.file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp_path = self.file_path.with_extension("json.tmp");
        if let Ok(bytes) = serde_json::to_vec_pretty(state) {
            if std::fs::write(&tmp_path, bytes).is_ok() {
                let _ = std::fs::rename(&tmp_path, &self.file_path);
            }
        }
    }
}

impl Default for KnowledgeStore {
    fn default() -> Self {
        Self::new()
    }
}

fn default_knowledge_scope() -> String {
    "__global__".to_string()
}

fn normalize_knowledge_scope(scope: Option<&str>) -> String {
    let scope = scope.unwrap_or("").trim();
    if scope.is_empty() {
        return default_knowledge_scope();
    }
    std::fs::canonicalize(scope)
        .unwrap_or_else(|_| PathBuf::from(scope))
        .to_string_lossy()
        .to_string()
}

fn active_docs_mut(state: &mut PersistedKnowledgeState) -> &mut HashMap<String, KnowledgeDocument> {
    let scope = state.active_scope.clone();
    state.scopes.entry(scope).or_default()
}

fn active_docs(state: &PersistedKnowledgeState) -> Option<&HashMap<String, KnowledgeDocument>> {
    state.scopes.get(&state.active_scope)
}

fn load_knowledge_state(path: &Path) -> PersistedKnowledgeState {
    let Ok(bytes) = std::fs::read(path) else {
        return PersistedKnowledgeState::default();
    };

    serde_json::from_slice::<PersistedKnowledgeState>(&bytes).unwrap_or_default()
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

fn canonicalize_if_exists(path: &Path) -> Option<PathBuf> {
    if !path.exists() {
        return None;
    }
    Some(std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
}

fn knowledge_metadata_text(name: &str, source_path: Option<&Path>) -> String {
    let mut aliases = vec![name.to_string(), normalize_knowledge_label(name)];

    if let Some(path) = source_path {
        let path_str = path.to_string_lossy().to_string();
        aliases.push(path_str);

        if let Some(file_name) = path.file_name().and_then(|value| value.to_str()) {
            aliases.push(file_name.to_string());
            aliases.push(normalize_knowledge_label(file_name));
        }

        if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
            aliases.push(stem.to_string());
            aliases.push(normalize_knowledge_label(stem));
        }
    }

    aliases.retain(|alias| !alias.trim().is_empty());
    aliases.sort();
    aliases.dedup();
    aliases.join("\n")
}

fn normalize_knowledge_label(label: &str) -> String {
    label
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn summarize_preview(text: &str, max_chars: usize) -> Option<String> {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return None;
    }

    let mut chars = compact.chars();
    let preview: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        Some(format!("{preview}..."))
    } else {
        Some(preview)
    }
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
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
            // If value is a file/dir path, read it; otherwise treat as text
            let source_path = canonicalize_if_exists(Path::new(value));
            let content = source_path
                .as_deref()
                .map(read_knowledge_source)
                .unwrap_or_else(|| value.to_string());
            let chunk_count = knowledge.add(name, &content, source_path.as_deref());
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
            let results: Vec<Value> = knowledge
                .search(query, limit)
                .iter()
                .map(|(name, chunk)| json!({"source": name, "content": truncate_text(chunk, 500)}))
                .collect();
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
            let source_path = canonicalize_if_exists(Path::new(path));
            let content = source_path
                .as_deref()
                .map(read_knowledge_source)
                .unwrap_or_default();
            let chunk_count = knowledge.add(n, &content, source_path.as_deref());
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
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("contextro-knowledge-{unique}-{name}"))
    }

    fn temp_file(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("contextro-knowledge-{unique}-{name}.json"))
    }

    fn temp_db(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("contextro-memory-{unique}-{name}.db"))
    }

    #[test]
    fn test_forget_accepts_id_alias_from_remember() {
        let db_path = temp_db("forget-id");
        let store = MemoryStore::new(db_path.to_string_lossy().as_ref()).unwrap();

        let remember_result =
            handle_remember(&json!({"content":"remember forget id alias"}), &store);
        let id = remember_result["id"].as_str().unwrap().to_string();

        let forget_result = handle_forget(&json!({"id": id}), &store);
        assert_eq!(forget_result["deleted"], 1);

        let recall_result = handle_recall(&json!({"query":"remember forget id alias"}), &store);
        assert_eq!(recall_result["total"], 0);

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn test_knowledge_list_alias_returns_indexed_bases() {
        let store_path = temp_file("list");
        let knowledge = KnowledgeStore::with_path(&store_path);
        assert_eq!(knowledge.add("docs", "chunk one\nchunk two", None), 1);

        let result = handle_knowledge(&json!({"command":"list"}), &knowledge);
        assert_eq!(result["total"], 1);
        assert_eq!(result["knowledge_bases"][0]["name"], "docs");
        assert_eq!(result["knowledge_bases"][0]["chunks"], 1);

        let _ = std::fs::remove_file(store_path);
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

        let store_path = temp_file("nested");
        let knowledge = KnowledgeStore::with_path(&store_path);
        knowledge.set_active_scope(Some(root.to_string_lossy().as_ref()));
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
        let _ = std::fs::remove_file(store_path);
    }

    #[test]
    fn test_knowledge_search_matches_manual_doc_name_for_high_level_queries() {
        let root = temp_dir("roadmap");
        let roadmap = root.join("ROADMAP.md");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            &roadmap,
            "Prioritize developer trust, launch quality, and release automation.",
        )
        .unwrap();

        let store_path = temp_file("roadmap");
        let knowledge = KnowledgeStore::with_path(&store_path);
        knowledge.set_active_scope(Some(root.to_string_lossy().as_ref()));
        let add_result = handle_knowledge(
            &json!({"command":"add","name":"ROADMAP.md","value": roadmap.to_string_lossy()}),
            &knowledge,
        );
        assert_eq!(add_result["status"], "indexed");

        let search_result = handle_knowledge(
            &json!({"command":"search","query":"roadmap priorities","limit":5}),
            &knowledge,
        );
        assert_eq!(search_result["total"], 1);
        assert_eq!(search_result["results"][0]["source"], "ROADMAP.md");

        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_file(store_path);
    }

    #[test]
    fn test_knowledge_show_returns_more_detail_than_list() {
        let root = temp_dir("show");
        let note = root.join("guide.md");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&note, "Guide preview text for knowledge show details.").unwrap();

        let store_path = temp_file("show");
        let knowledge = KnowledgeStore::with_path(&store_path);
        knowledge.set_active_scope(Some(root.to_string_lossy().as_ref()));
        handle_knowledge(
            &json!({"command":"add","name":"guide","value": note.to_string_lossy()}),
            &knowledge,
        );

        let show_result = handle_knowledge(&json!({"command":"show"}), &knowledge);
        let list_result = handle_knowledge(&json!({"command":"list"}), &knowledge);

        assert_eq!(show_result["knowledge_bases"][0]["name"], "guide");
        assert!(show_result["knowledge_bases"][0]["preview"]
            .as_str()
            .unwrap()
            .contains("Guide preview text"));
        assert!(show_result["knowledge_bases"][0]["source_path"]
            .as_str()
            .unwrap()
            .ends_with("guide.md"));
        assert!(list_result["knowledge_bases"][0].get("preview").is_none());
        assert!(list_result["knowledge_bases"][0]
            .get("source_path")
            .is_none());

        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_file(store_path);
    }

    #[test]
    fn test_knowledge_search_truncates_unicode_safely() {
        let root = temp_dir("unicode");
        let note = root.join("guide.md");
        std::fs::create_dir_all(&root).unwrap();
        let unicode_text = format!("{}match token", "─".repeat(600));
        std::fs::write(&note, &unicode_text).unwrap();

        let store_path = temp_file("unicode");
        let knowledge = KnowledgeStore::with_path(&store_path);
        knowledge.set_active_scope(Some(root.to_string_lossy().as_ref()));
        handle_knowledge(
            &json!({"command":"add","name":"guide","value": note.to_string_lossy()}),
            &knowledge,
        );

        let search_result = handle_knowledge(
            &json!({"command":"search","query":"match token","limit":5}),
            &knowledge,
        );

        assert_eq!(search_result["total"], 1);
        let content = search_result["results"][0]["content"].as_str().unwrap();
        assert!(content.ends_with("..."));
        assert!(content.chars().count() <= 503);

        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_file(store_path);
    }

    #[test]
    fn test_knowledge_persists_active_scope_across_reloads() {
        let store_path = temp_file("persist");
        let root = temp_dir("persist-root");
        let roadmap = root.join("ROADMAP.md");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            &roadmap,
            "Roadmap priorities: developer trust and milestone discipline.",
        )
        .unwrap();

        let knowledge = KnowledgeStore::with_path(&store_path);
        knowledge.set_active_scope(Some(root.to_string_lossy().as_ref()));
        let add_result = handle_knowledge(
            &json!({"command":"add","name":"ROADMAP.md","value": roadmap.to_string_lossy()}),
            &knowledge,
        );
        assert_eq!(add_result["status"], "indexed");
        drop(knowledge);

        let reloaded = KnowledgeStore::with_path(&store_path);
        let list_result = handle_knowledge(&json!({"command":"list"}), &reloaded);
        let search_result = handle_knowledge(
            &json!({"command":"search","query":"roadmap priorities","limit":5}),
            &reloaded,
        );

        assert_eq!(list_result["total"], 1);
        assert_eq!(list_result["knowledge_bases"][0]["name"], "ROADMAP.md");
        assert_eq!(search_result["total"], 1);
        assert_eq!(search_result["results"][0]["source"], "ROADMAP.md");

        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_file(store_path);
    }

    #[test]
    fn test_knowledge_scopes_isolate_same_named_docs() {
        let store_path = temp_file("scopes");
        let root_a = temp_dir("scope-a");
        let root_b = temp_dir("scope-b");
        std::fs::create_dir_all(&root_a).unwrap();
        std::fs::create_dir_all(&root_b).unwrap();

        let knowledge = KnowledgeStore::with_path(&store_path);
        knowledge.set_active_scope(Some(root_a.to_string_lossy().as_ref()));
        assert_eq!(
            knowledge.add("README.md", "alpha release checklist", None),
            1
        );

        knowledge.set_active_scope(Some(root_b.to_string_lossy().as_ref()));
        assert_eq!(knowledge.add("README.md", "beta browser workflow", None), 1);

        let scope_b = handle_knowledge(
            &json!({"command":"search","query":"browser workflow","limit":5}),
            &knowledge,
        );
        assert_eq!(scope_b["total"], 1);

        knowledge.set_active_scope(Some(root_a.to_string_lossy().as_ref()));
        let scope_a = handle_knowledge(
            &json!({"command":"search","query":"release checklist","limit":5}),
            &knowledge,
        );
        let scope_a_miss = handle_knowledge(
            &json!({"command":"search","query":"browser workflow","limit":5}),
            &knowledge,
        );
        assert_eq!(scope_a["total"], 1);
        assert_eq!(scope_a_miss["total"], 0);

        let _ = std::fs::remove_dir_all(root_a);
        let _ = std::fs::remove_dir_all(root_b);
        let _ = std::fs::remove_file(store_path);
    }
}
