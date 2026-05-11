//! Git tools: commit_search, commit_history, repo_add, repo_remove, repo_status.

use std::collections::HashMap;
use std::path::Path;

use parking_lot::RwLock;
use serde_json::{json, Value};

/// Registered repos tracker.
pub struct RepoRegistry {
    repos: RwLock<HashMap<String, String>>, // path -> name
}

impl RepoRegistry {
    pub fn new() -> Self {
        Self { repos: RwLock::new(HashMap::new()) }
    }

    pub fn add(&self, path: &str, name: Option<&str>) -> bool {
        let n = name.unwrap_or_else(|| Path::new(path).file_name().unwrap_or_default().to_str().unwrap_or("repo"));
        self.repos.write().insert(path.to_string(), n.to_string());
        true
    }

    pub fn remove(&self, path: &str) -> bool {
        self.repos.write().remove(path).is_some()
    }

    pub fn list(&self) -> Vec<(String, String)> {
        self.repos.read().iter().map(|(p, n)| (p.clone(), n.clone())).collect()
    }
}

impl Default for RepoRegistry {
    fn default() -> Self {
        Self::new()
    }
}

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

/// Semantic commit search: tokenize query and score commits by token overlap.
pub fn handle_commit_search(args: &Value, codebase: Option<&str>) -> Value {
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
    if query.is_empty() {
        return json!({"error": "Missing required parameter: query"});
    }
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let author_filter = args.get("author").and_then(|v| v.as_str());
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

    // Tokenize query for fuzzy matching
    let query_tokens: Vec<String> = tokenize(query);

    let mut scored_commits: Vec<(f64, Value)> = revwalk
        .take(500)
        .filter_map(|oid| {
            let oid = oid.ok()?;
            let commit = repo.find_commit(oid).ok()?;
            let msg = commit.message().unwrap_or("").to_string();
            let sig = commit.author();
            let author = sig.name().unwrap_or("").to_string();

            // Author filter
            if let Some(af) = author_filter {
                if !author.to_lowercase().contains(&af.to_lowercase()) {
                    return None;
                }
            }

            // Score by token overlap
            let msg_tokens = tokenize(&msg);
            let score = token_overlap_score(&query_tokens, &msg_tokens);
            if score > 0.0 {
                Some((score, json!({
                    "hash": oid.to_string()[..12].to_string(),
                    "message": msg.lines().next().unwrap_or("").to_string(),
                    "author": author,
                    "score": (score * 100.0).round() / 100.0,
                })))
            } else {
                None
            }
        })
        .collect();

    scored_commits.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored_commits.truncate(limit);

    let commits: Vec<Value> = scored_commits.into_iter().map(|(_, v)| v).collect();
    json!({"query": query, "commits": commits, "total": commits.len()})
}

pub fn handle_repo_add(args: &Value, registry: &RepoRegistry) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return json!({"error": "Missing required parameter: path"});
    }
    if !Path::new(path).is_dir() {
        return json!({"error": format!("Not a directory: {}", path)});
    }
    let name = args.get("name").and_then(|v| v.as_str());
    registry.add(path, name);
    json!({"registered": true, "path": path})
}

pub fn handle_repo_remove(args: &Value, registry: &RepoRegistry) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return json!({"error": "Missing required parameter: path"});
    }
    let removed = registry.remove(path);
    json!({"removed": removed, "path": path})
}

pub fn handle_repo_status(registry: &RepoRegistry) -> Value {
    let repos: Vec<Value> = registry.list().iter().map(|(path, name)| {
        let is_git = git2::Repository::discover(path).is_ok();
        json!({"path": path, "name": name, "is_git": is_git})
    }).collect();
    json!({"repos": repos, "total": repos.len()})
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| s.len() > 2)
        .map(String::from)
        .collect()
}

fn token_overlap_score(query_tokens: &[String], doc_tokens: &[String]) -> f64 {
    if query_tokens.is_empty() || doc_tokens.is_empty() {
        return 0.0;
    }
    let matches = query_tokens.iter().filter(|qt| {
        doc_tokens.iter().any(|dt| dt.contains(qt.as_str()) || qt.contains(dt.as_str()))
    }).count();
    matches as f64 / query_tokens.len() as f64
}
