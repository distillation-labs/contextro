use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use contextro_config::get_settings;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
mod context;

use context::commit_diff_context_score;

use serde_json::{json, Value};

use super::{load_repos, normalize_repo_path, token_overlap_score_lower, tokenize};

#[derive(Clone)]
struct CachedCommitRecord {
    oid: git2::Oid,
    hash: String,
    message: String,
    message_lower: String,
    author: String,
    author_lower: String,
    diff_context: Arc<OnceLock<CommitDiffContext>>,
    tokens: Vec<String>,
}

#[derive(Clone, Default)]
struct CommitDiffContext {
    text_lower: String,
    tokens: Vec<String>,
}

#[derive(Clone)]
pub(crate) struct CommitSearchCacheEntry {
    pub(crate) head_hash: String,
    pub(crate) scan_limit: usize,
    records: Arc<Vec<CachedCommitRecord>>,
}

static COMMIT_SEARCH_CACHE: OnceLock<RwLock<HashMap<String, CommitSearchCacheEntry>>> =
    OnceLock::new();
static COMMIT_SEARCH_RESULT_CACHE: OnceLock<RwLock<HashMap<String, Value>>> = OnceLock::new();
static COMMIT_SEARCH_PREWARM_INFLIGHT: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();

pub(crate) fn commit_search_cache() -> &'static RwLock<HashMap<String, CommitSearchCacheEntry>> {
    COMMIT_SEARCH_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

pub(crate) fn commit_search_result_cache() -> &'static RwLock<HashMap<String, Value>> {
    COMMIT_SEARCH_RESULT_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn commit_search_prewarm_inflight() -> &'static RwLock<HashSet<String>> {
    COMMIT_SEARCH_PREWARM_INFLIGHT.get_or_init(|| RwLock::new(HashSet::new()))
}

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
        self.remove_entry(Some(path), None).is_some()
    }

    pub fn remove_by_name(&self, name: &str) -> bool {
        self.remove_entry(None, Some(name)).is_some()
    }

    pub fn remove_entry(&self, path: Option<&str>, name: Option<&str>) -> Option<(String, String)> {
        let mut repos = self.repos.write();
        let removed = if let Some(path) = path.filter(|path| !path.is_empty()) {
            let key = normalize_repo_path(path);
            repos
                .remove_entry(&key)
                .or_else(|| repos.remove_entry(path))
        } else if let Some(name) = name.filter(|name| !name.is_empty()) {
            let matching_key = repos
                .iter()
                .find_map(|(path, stored_name)| (stored_name == name).then(|| path.clone()));
            matching_key.and_then(|key| repos.remove_entry(&key))
        } else {
            None
        };

        if removed.is_some() {
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
        if let Ok(bytes) = serde_json::to_vec(&payload) {
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
pub(crate) struct StoredRepo {
    pub(crate) path: String,
    pub(crate) name: String,
}

pub fn prewarm_commit_search_cache(codebase: Option<&str>) {
    let Some(repo_path) = codebase else {
        return;
    };
    let Ok(repo) = git2::Repository::discover(repo_path) else {
        return;
    };

    let initial_scan_limit = get_settings().read().commit_history_limit.max(500);
    let repo_key = commit_search_repo_key(&repo);
    let head_hash = commit_search_head_hash(&repo);

    if let Some(entry) = commit_search_cache().read().get(&repo_key) {
        if entry.head_hash == head_hash && entry.scan_limit >= initial_scan_limit {
            return;
        }
    }

    {
        let mut inflight = commit_search_prewarm_inflight().write();
        if !inflight.insert(repo_key.clone()) {
            return;
        }
    }

    let repo_path = repo_path.to_string();
    std::thread::spawn(move || {
        if let Ok(repo) = git2::Repository::discover(&repo_path) {
            let _ = load_commit_search_records(&repo, initial_scan_limit);
        }
        commit_search_prewarm_inflight().write().remove(&repo_key);
    });
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

    let query_tokens: Vec<String> = tokenize(query);
    let query_lower = query.to_ascii_lowercase();
    let author_filter_lower = author_filter.map(|author| author.to_ascii_lowercase());
    let repo_key = commit_search_repo_key(&repo);
    let head_hash = commit_search_head_hash(&repo);
    let result_cache_key = commit_search_result_cache_key(
        &repo_key,
        &head_hash,
        &query_lower,
        author_filter_lower.as_deref(),
        limit,
    );
    if let Some(cached) = commit_search_result_cache()
        .read()
        .get(&result_cache_key)
        .cloned()
    {
        return cached;
    }
    let initial_scan_limit = get_settings().read().commit_history_limit.max(500);
    let fallback_scan_limit = initial_scan_limit.max(5000);
    let records = load_commit_search_records(&repo, initial_scan_limit);

    let mut scored_commits = score_commit_records(
        &repo,
        records.iter().take(initial_scan_limit),
        &query_lower,
        &query_tokens,
        author_filter_lower.as_deref(),
    );
    if scored_commits.is_empty() && fallback_scan_limit > initial_scan_limit {
        let records = load_commit_search_records(&repo, fallback_scan_limit);
        scored_commits = score_commit_records(
            &repo,
            records.iter(),
            &query_lower,
            &query_tokens,
            author_filter_lower.as_deref(),
        );
    }

    let min_score = commit_search_min_score(query, &query_tokens);
    scored_commits.retain(|(score, _)| *score >= min_score);
    scored_commits.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored_commits.truncate(limit);

    let commits: Vec<Value> = scored_commits.into_iter().map(|(_, v)| v).collect();
    let response = json!({"query": query, "commits": commits, "total": commits.len()});
    store_commit_search_result_cache_entry(result_cache_key, &response);
    response
}

fn commit_search_min_score(query: &str, query_tokens: &[String]) -> f64 {
    if query_tokens.is_empty() {
        return 1.0;
    }
    if query.contains('_') || query.contains('-') || query_tokens.len() >= 3 {
        0.45
    } else {
        0.18
    }
}

fn load_commit_search_records(
    repo: &git2::Repository,
    scan_limit: usize,
) -> Arc<Vec<CachedCommitRecord>> {
    let repo_key = commit_search_repo_key(repo);
    let head_hash = commit_search_head_hash(repo);

    if let Some(entry) = commit_search_cache().read().get(&repo_key) {
        if entry.head_hash == head_hash && entry.scan_limit >= scan_limit {
            return Arc::clone(&entry.records);
        }
    }

    let records = Arc::new(scan_commit_records(repo, scan_limit));
    commit_search_cache().write().insert(
        repo_key,
        CommitSearchCacheEntry {
            head_hash,
            scan_limit,
            records: Arc::clone(&records),
        },
    );
    records
}

fn scan_commit_records(repo: &git2::Repository, scan_limit: usize) -> Vec<CachedCommitRecord> {
    let mut revwalk = match repo.revwalk() {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    if revwalk.push_head().is_err() {
        return Vec::new();
    }

    revwalk
        .take(scan_limit)
        .filter_map(|oid| {
            let oid = oid.ok()?;
            let commit = repo.find_commit(oid).ok()?;
            let message = commit.message().unwrap_or("").to_string();
            let author = commit.author().name().unwrap_or("").to_string();
            Some(CachedCommitRecord {
                oid,
                hash: oid.to_string()[..12].to_string(),
                message_lower: message.to_lowercase(),
                diff_context: Arc::new(OnceLock::new()),
                tokens: tokenize(&message),
                message,
                author_lower: author.to_lowercase(),
                author,
            })
        })
        .collect()
}

fn score_commit_records<'a, I>(
    repo: &git2::Repository,
    records: I,
    query_lower: &str,
    query_tokens: &[String],
    author_filter_lower: Option<&str>,
) -> Vec<(f64, Value)>
where
    I: IntoIterator<Item = &'a CachedCommitRecord>,
{
    records
        .into_iter()
        .filter_map(|record| {
            if let Some(author_filter_lower) = author_filter_lower {
                if !record.author_lower.contains(author_filter_lower) {
                    return None;
                }
            }

            let message_score = token_overlap_score_lower(
                query_lower,
                query_tokens,
                &record.message_lower,
                &record.tokens,
            );
            let context_score = if message_score > 0.0 {
                0.0
            } else {
                commit_diff_context_score(repo, record, query_lower, query_tokens)
            };
            let score = merge_commit_search_scores(message_score, context_score);
            if score > 0.0 {
                Some((
                    score,
                    json!({
                        "hash": record.hash,
                        "message": record.message.lines().next().unwrap_or("").to_string(),
                        "author": record.author,
                        "score": (score * 100.0).round() / 100.0,
                    }),
                ))
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn commit_search_repo_key(repo: &git2::Repository) -> String {
    repo.workdir()
        .or_else(|| repo.path().parent())
        .map(|path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
        .unwrap_or_else(|| repo.path().to_path_buf())
        .to_string_lossy()
        .to_string()
}

pub(crate) fn commit_search_head_hash(repo: &git2::Repository) -> String {
    repo.head()
        .ok()
        .and_then(|head| head.target())
        .map(|oid| oid.to_string())
        .unwrap_or_default()
}

pub(crate) fn commit_search_result_cache_key(
    repo_key: &str,
    head_hash: &str,
    query_lower: &str,
    author_filter_lower: Option<&str>,
    limit: usize,
) -> String {
    format!(
        "{repo_key}\u{1f}{head_hash}\u{1f}{}\u{1f}{limit}\u{1f}{query_lower}",
        author_filter_lower.unwrap_or("")
    )
}

fn store_commit_search_result_cache_entry(cache_key: String, response: &Value) {
    let mut cache = commit_search_result_cache().write();
    if cache.len() >= 256 {
        cache.clear();
    }
    cache.insert(cache_key, response.clone());
}

fn merge_commit_search_scores(message_score: f64, context_score: f64) -> f64 {
    if message_score > 0.0 {
        message_score
    } else {
        context_score * 0.70
    }
}
