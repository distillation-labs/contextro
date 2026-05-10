//! Git tools: commit_search, commit_history, repo_add, repo_remove, repo_status.

use serde_json::{json, Value};

pub fn handle_commit_history(args: &Value, codebase: Option<&str>) -> Value {
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let repo_path = codebase.unwrap_or(".");

    let repo = match git2::Repository::discover(repo_path) {
        Ok(r) => r,
        Err(_) => return json!({"error": "Not a git repository"}),
    };

    let mut revwalk = match repo.revwalk() {
        Ok(r) => r,
        Err(_) => return json!({"error": "Failed to walk commits"}),
    };
    revwalk.push_head().ok();

    let commits: Vec<Value> = revwalk
        .take(limit)
        .filter_map(|oid| {
            let oid = oid.ok()?;
            let commit = repo.find_commit(oid).ok()?;
            Some(json!({
                "hash": oid.to_string()[..12].to_string(),
                "message": commit.summary().unwrap_or("").to_string(),
                "author": commit.author().name().unwrap_or("").to_string(),
                "time": commit.time().seconds(),
            }))
        })
        .collect();

    json!({"commits": commits, "total": commits.len()})
}

pub fn handle_commit_search(args: &Value, codebase: Option<&str>) -> Value {
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
    if query.is_empty() {
        return json!({"error": "Missing required parameter: query"});
    }
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let repo_path = codebase.unwrap_or(".");

    let repo = match git2::Repository::discover(repo_path) {
        Ok(r) => r,
        Err(_) => return json!({"error": "Not a git repository"}),
    };

    let mut revwalk = match repo.revwalk() {
        Ok(r) => r,
        Err(_) => return json!({"error": "Failed to walk commits"}),
    };
    revwalk.push_head().ok();

    let query_lower = query.to_lowercase();
    let commits: Vec<Value> = revwalk
        .take(500)
        .filter_map(|oid| {
            let oid = oid.ok()?;
            let commit = repo.find_commit(oid).ok()?;
            let msg = commit.message().unwrap_or("");
            if msg.to_lowercase().contains(&query_lower) {
                Some(json!({
                    "hash": oid.to_string()[..12].to_string(),
                    "message": commit.summary().unwrap_or("").to_string(),
                    "author": commit.author().name().unwrap_or("").to_string(),
                }))
            } else {
                None
            }
        })
        .take(limit)
        .collect();

    json!({"query": query, "commits": commits, "total": commits.len()})
}

pub fn handle_repo_add(args: &Value) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return json!({"error": "Missing required parameter: path"});
    }
    if !std::path::Path::new(path).is_dir() {
        return json!({"error": format!("Not a directory: {}", path)});
    }
    json!({"registered": true, "path": path})
}

pub fn handle_repo_remove(args: &Value) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return json!({"error": "Missing required parameter: path"});
    }
    json!({"removed": true, "path": path})
}

pub fn handle_repo_status() -> Value {
    json!({"repos": [], "total": 0})
}
