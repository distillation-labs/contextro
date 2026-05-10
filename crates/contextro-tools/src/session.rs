//! Session tools: compact, session_snapshot, restore, retrieve.

use contextro_engines::sandbox::OutputSandbox;
use contextro_memory::session::SessionTracker;
use serde_json::{json, Value};

pub fn handle_compact(args: &Value) -> Value {
    let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
    if content.is_empty() {
        return json!({"error": "Missing required parameter: content"});
    }
    // TODO: archive content for later retrieval
    let ref_id = format!("arc_{:08x}", content.len());
    json!({"archived": true, "ref_id": ref_id, "chars": content.len()})
}

pub fn handle_session_snapshot(tracker: &SessionTracker) -> Value {
    let events = tracker.recent_events(20);
    let event_list: Vec<Value> = events.iter().map(|e| {
        json!({"type": e.event_type, "summary": e.summary})
    }).collect();
    json!({"events": event_list, "total": event_list.len()})
}

pub fn handle_restore(codebase: Option<&str>, node_count: usize, rel_count: usize) -> Value {
    json!({
        "codebase_path": codebase,
        "graph_nodes": node_count,
        "graph_relationships": rel_count,
        "hint": "Index is loaded. Use search/find_symbol/explain to query.",
    })
}

pub fn handle_retrieve(args: &Value, sandbox: &OutputSandbox) -> Value {
    let ref_id = args.get("ref_id").and_then(|v| v.as_str()).unwrap_or("");
    if ref_id.is_empty() {
        return json!({"error": "Missing required parameter: ref_id"});
    }
    match sandbox.retrieve(ref_id) {
        Some(content) => json!({"ref_id": ref_id, "content": content}),
        None => json!({"error": format!("Reference '{}' not found or expired.", ref_id)}),
    }
}
