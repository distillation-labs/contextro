//! Memory tools: remember, recall, forget, knowledge.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use contextro_config::get_settings;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use contextro_core::models::{Memory, MemoryTtl, MemoryType};
use contextro_memory::store::MemoryStore;

mod knowledge_commands;
mod knowledge_state;
mod knowledge_store;

pub use knowledge_commands::handle_knowledge;
pub use knowledge_store::{KnowledgeDocSummary, KnowledgeStore};

pub fn handle_remember(args: &Value, store: &MemoryStore) -> Value {
    let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
    if content.is_empty() {
        return json!({"error": "Missing required parameter: content"});
    }
    let memory_type = match parse_memory_type_arg(
        args.get("memory_type")
            .and_then(|v| v.as_str())
            .unwrap_or("note"),
    ) {
        Ok(memory_type) => memory_type,
        Err(error) => return json!({"error": error}),
    };
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
    let ttl = match parse_ttl_arg(
        args.get("ttl")
            .and_then(|v| v.as_str())
            .unwrap_or("permanent"),
    ) {
        Ok(ttl) => ttl,
        Err(error) => return json!({"error": error}),
    };
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
        Ok(id) => json!({
            "stored": true,
            "id": id,
            "memory_type": memory_type.to_string(),
            "tags": tags,
            "ttl": ttl_name(ttl),
            "expires_at": ttl_expires_at(&memory.created_at, ttl),
        }),
        Err(e) => json!({"error": format!("Failed to store: {}", e)}),
    }
}

pub fn handle_recall(args: &Value, store: &MemoryStore) -> Value {
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
    let memory_type = args.get("memory_type").and_then(|v| v.as_str());
    let tags_owned = string_list_arg(args.get("tags"));
    let tags = tags_owned.as_deref();

    match store.recall(query, limit, memory_type, tags, None) {
        Ok(memories) => {
            let results: Vec<Value> = memories
                .iter()
                .map(|m| {
                    json!({
                        "id": m.id,
                        "content": m.content,
                        "type": m.memory_type.to_string(),
                        "tags": m.tags,
                        "created_at": m.created_at,
                        "ttl": ttl_name(m.ttl),
                        "expires_at": ttl_expires_at(&m.created_at, m.ttl),
                    })
                })
                .collect();
            json!({
                "query": if query.is_empty() { Value::Null } else { json!(query) },
                "memories": results,
                "total": results.len(),
                "limit": limit,
                "memory_type": memory_type,
                "tags": tags,
            })
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
        Ok(0) if id.is_some() => {
            json!({"error": format!("Memory '{}' not found.", id.unwrap_or_default())})
        }
        Ok(n) => json!({"deleted": n}),
        Err(e) => json!({"error": format!("Forget failed: {}", e)}),
    }
}

fn parse_memory_type_arg(s: &str) -> Result<MemoryType, String> {
    match s {
        "note" => Ok(MemoryType::Note),
        "conversation" => Ok(MemoryType::Conversation),
        "status" => Ok(MemoryType::Status),
        "decision" => Ok(MemoryType::Decision),
        "preference" => Ok(MemoryType::Preference),
        "doc" => Ok(MemoryType::Doc),
        other => Err(format!(
            "Invalid memory_type: '{}'. Expected one of: note, decision, preference, conversation, status, doc",
            other
        )),
    }
}

fn parse_ttl_arg(s: &str) -> Result<MemoryTtl, String> {
    match s {
        "permanent" => Ok(MemoryTtl::Permanent),
        "session" => Ok(MemoryTtl::Session),
        "day" => Ok(MemoryTtl::Day),
        "week" => Ok(MemoryTtl::Week),
        "month" => Ok(MemoryTtl::Month),
        other => Err(format!(
            "Invalid ttl: '{}'. Expected one of: permanent, session, day, week, month",
            other
        )),
    }
}

fn ttl_name(ttl: MemoryTtl) -> &'static str {
    match ttl {
        MemoryTtl::Permanent => "permanent",
        MemoryTtl::Session => "session",
        MemoryTtl::Day => "day",
        MemoryTtl::Week => "week",
        MemoryTtl::Month => "month",
    }
}

fn ttl_duration(ttl: MemoryTtl) -> Option<Duration> {
    match ttl {
        MemoryTtl::Permanent => None,
        MemoryTtl::Session => Some(Duration::hours(4)),
        MemoryTtl::Day => Some(Duration::days(1)),
        MemoryTtl::Week => Some(Duration::weeks(1)),
        MemoryTtl::Month => Some(Duration::days(30)),
    }
}

fn ttl_expires_at(created_at: &str, ttl: MemoryTtl) -> Option<String> {
    let created_at = DateTime::parse_from_rfc3339(created_at).ok()?;
    let duration = ttl_duration(ttl)?;
    Some((created_at.with_timezone(&Utc) + duration).to_rfc3339())
}

fn string_list_arg(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::Array(arr)) => {
            let joined = arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(",");
            if joined.is_empty() {
                None
            } else {
                Some(joined)
            }
        }
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
        _ => None,
    }
}

#[cfg(test)]
#[cfg(test)]
mod tests;
