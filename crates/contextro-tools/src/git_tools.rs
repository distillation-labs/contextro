//! Git tools: commit_search, commit_history, repo_add, repo_remove, repo_status.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use contextro_config::get_settings;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Registered repos tracker.
pub struct RepoRegistry {
    repos: RwLock<HashMap<String, String>>, // path -> name
    file_path: PathBuf,
}

impl RepoRegistry {
    pub fn new() -> Self {
        let storage_dir = get_settings().read().storage_dir.clone();
        Self::with_path(PathBuf::from(storage_dir).join("repo-registry.json"))
    }

    pub fn with_path<P: Into<PathBuf>>(file_path: P) -> Self {
        let file_path = file_path.into();
        Self {
            repos: RwLock::new(load_repos(&file_path)),
            file_path,
        }
    }

    pub fn add(&self, path: &str, name: Option<&str>) -> bool {
        let key = normalize_repo_path(path);
        let n = name.unwrap_or_else(|| {
            Path::new(path)
                .file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or("repo")
        });
        let mut repos = self.repos.write();
        repos.insert(key, n.to_string());
        self.save_locked(&repos);
        true
    }

    pub fn remove(&self, path: &str) -> bool {
        let key = normalize_repo_path(path);
        let mut repos = self.repos.write();
        let removed = repos.remove(&key).is_some() || repos.remove(path).is_some();
        if removed {
            self.save_locked(&repos);
        }
        removed
    }

    pub fn list(&self) -> Vec<(String, String)> {
        let mut repos: Vec<(String, String)> = self
            .repos
            .read()
            .iter()
            .map(|(p, n)| (p.clone(), n.clone()))
            .collect();
        repos.sort_by(|a, b| a.0.cmp(&b.0));
        repos
    }

    fn save_locked(&self, repos: &HashMap<String, String>) {
        if let Some(parent) = self.file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp_path = self.file_path.with_extension("json.tmp");
        let payload: Vec<StoredRepo> = repos
            .iter()
            .map(|(path, name)| StoredRepo {
                path: path.clone(),
                name: name.clone(),
            })
            .collect();
        if let Ok(bytes) = serde_json::to_vec_pretty(&payload) {
            if std::fs::write(&tmp_path, bytes).is_ok() {
                let _ = std::fs::rename(&tmp_path, &self.file_path);
            }
        }
    }
}

impl Default for RepoRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredRepo {
    path: String,
    name: String,
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
                Some((
                    score,
                    json!({
                        "hash": oid.to_string()[..12].to_string(),
                        "message": msg.lines().next().unwrap_or("").to_string(),
                        "author": author,
                        "score": (score * 100.0).round() / 100.0,
                    }),
                ))
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
    json!({"registered": true, "path": normalize_repo_path(path), "hint": "Run index(path) to build the graph and enable search for this repo."})
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
    let repos: Vec<Value> = registry
        .list()
        .iter()
        .map(|(path, name)| {
            let is_git = git2::Repository::discover(path).is_ok();
            json!({"path": path, "name": name, "is_git": is_git})
        })
        .collect();
    json!({"repos": repos, "total": repos.len()})
}

fn normalize_repo_path(path: &str) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| PathBuf::from(path))
        .to_string_lossy()
        .to_string()
}

fn load_repos(path: &Path) -> HashMap<String, String> {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Vec<StoredRepo>>(&bytes).ok())
        .map(|repos| {
            repos
                .into_iter()
                .map(|repo| (repo.path, repo.name))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default()
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
    let matches = query_tokens
        .iter()
        .filter(|qt| {
            doc_tokens
                .iter()
                .any(|dt| dt.contains(qt.as_str()) || qt.contains(dt.as_str()))
        })
        .count();
    matches as f64 / query_tokens.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("contextro-repos-{unique}-{name}"))
    }

    #[test]
    fn test_repo_registry_persists_to_disk() {
        let path = temp_file("repos.json");
        let repo_dir = std::env::temp_dir().join("contextro-repo-registry-test");
        let _ = std::fs::create_dir_all(&repo_dir);

        let registry = RepoRegistry::with_path(&path);
        assert!(registry.add(repo_dir.to_string_lossy().as_ref(), Some("repo")));

        let reloaded = RepoRegistry::with_path(&path);
        let repos = reloaded.list();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].1, "repo");

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir_all(repo_dir);
    }
}
