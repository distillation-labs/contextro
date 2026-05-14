//! Session tools: compact, session_snapshot, restore, retrieve.

use chrono::{Duration, Utc};
use contextro_core::models::MemoryTtl;
use contextro_memory::archive::CompactionArchive;
use contextro_memory::session::SessionTracker;
use serde_json::{json, Value};

pub fn handle_compact(args: &Value, archive: &CompactionArchive) -> Value {
    let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
    if content.is_empty() {
        return json!({"error": "Missing required parameter: content"});
    }
    let ttl = args
        .get("ttl")
        .and_then(|v| v.as_str())
        .map(parse_ttl)
        .transpose();
    let ttl = match ttl {
        Ok(ttl) => ttl.unwrap_or(MemoryTtl::Permanent),
        Err(error) => return json!({"error": error}),
    };
    let ref_id = archive.archive(content, args.get("metadata").cloned());
    json!({
        "archived": true,
        "ref_id": ref_id,
        "chars": content.len(),
        "ttl": ttl_name(ttl),
        "expires_at": ttl_expires_at(ttl),
        "ttl_note": "compact refs currently use the archive's configured retention window; ttl is reported for observability and future compatibility.",
    })
}

pub fn handle_session_snapshot(args: &Value, tracker: &SessionTracker) -> Value {
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
    let event_type = args.get("type").and_then(|v| v.as_str());
    let events = tracker.recent_events_filtered(limit, event_type);
    let event_list: Vec<Value> = events
        .iter()
        .map(|e| {
            let mut event = json!({
                "type": e.event_type,
                "summary": e.summary,
                "timestamp": e.timestamp,
            });
            if let Some(args) = &e.arguments {
                event["arguments"] = args.clone();
            }
            event
        })
        .collect();
    json!({
        "events": event_list,
        "total": event_list.len(),
        "limit": limit,
        "type": event_type,
    })
}

pub fn handle_restore(codebase: Option<&str>, indexed: bool, node_count: usize, rel_count: usize) -> Value {
    json!({
        "codebase_path": codebase,
        "indexed": indexed,
        "graph_nodes": node_count,
        "graph_relationships": rel_count,
        "requires_index": !indexed,
        "hint": if indexed {
            "Index is loaded. Use search/find_symbol/explain to query."
        } else {
            "No searchable index is loaded in this MCP process. Run index(path) before symbol and graph tools."
        },
    })
}

pub fn handle_retrieve(args: &Value, archive: &CompactionArchive) -> Value {
    let ref_id = args.get("ref_id").and_then(|v| v.as_str()).unwrap_or("");
    if ref_id.is_empty() {
        return json!({"error": "Missing required parameter: ref_id"});
    }
    match archive.retrieve(ref_id) {
        Some(content) => json!({"ref_id": ref_id, "content": content}),
        None => json!({"error": format!("Reference '{}' not found or expired.", ref_id)}),
    }
}

fn parse_ttl(value: &str) -> Result<MemoryTtl, String> {
    match value {
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

fn ttl_expires_at(ttl: MemoryTtl) -> Option<String> {
    let duration = match ttl {
        MemoryTtl::Permanent => None,
        MemoryTtl::Session => Some(Duration::hours(4)),
        MemoryTtl::Day => Some(Duration::days(1)),
        MemoryTtl::Week => Some(Duration::weeks(1)),
        MemoryTtl::Month => Some(Duration::days(30)),
    }?;
    Some((Utc::now() + duration).to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;
    use contextro_memory::archive::CompactionArchive;
    use contextro_memory::session::SessionTracker;
    use std::path::PathBuf;
    use std::time::{Duration as StdDuration, SystemTime, UNIX_EPOCH};

    fn temp_file(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("contextro-session-tool-{unique}-{name}"))
    }

    #[test]
    fn test_session_snapshot_applies_limit_and_type_filters() {
        let path = temp_file("events.json");
        let tracker = SessionTracker::with_path(20, &path);
        tracker.track("index", "index(path=\"repo\")", None);
        tracker.track("search", "search(query=\"jwt\")", None);
        tracker.track("search", "search(query=\"cache\")", None);

        let result = handle_session_snapshot(&json!({"limit":1,"type":"search"}), &tracker);

        assert_eq!(result["total"], 1);
        assert_eq!(result["events"][0]["type"], "search");
        assert!(result["events"][0]["summary"].as_str().unwrap().contains("cache"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_compact_reports_ttl_observability() {
        let path = temp_file("archive.json");
        let archive = CompactionArchive::with_path(&path, 20, StdDuration::from_secs(86400));

        let result = handle_compact(&json!({"content":"session summary","ttl":"day"}), &archive);

        assert_eq!(result["ttl"], "day");
        assert!(result["expires_at"].as_str().is_some());
        assert!(result["ttl_note"].as_str().is_some());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_restore_reports_requires_index_when_not_loaded() {
        let result = handle_restore(None, false, 0, 0);

        assert_eq!(result["indexed"], false);
        assert_eq!(result["requires_index"], true);
        assert!(result["hint"].as_str().unwrap().contains("Run index(path)"));
    }
}
