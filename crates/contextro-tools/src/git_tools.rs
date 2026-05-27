//! Git tools: commit_search, commit_history, repo_add, repo_remove, repo_status.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use chrono::{DateTime, Utc};
use contextro_config::get_settings;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
struct CommitSearchCacheEntry {
    head_hash: String,
    scan_limit: usize,
    records: Arc<Vec<CachedCommitRecord>>,
}

static COMMIT_SEARCH_CACHE: OnceLock<RwLock<HashMap<String, CommitSearchCacheEntry>>> =
    OnceLock::new();
static COMMIT_SEARCH_RESULT_CACHE: OnceLock<RwLock<HashMap<String, Value>>> = OnceLock::new();
static COMMIT_SEARCH_PREWARM_INFLIGHT: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();

fn commit_search_cache() -> &'static RwLock<HashMap<String, CommitSearchCacheEntry>> {
    COMMIT_SEARCH_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn commit_search_result_cache() -> &'static RwLock<HashMap<String, Value>> {
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
struct StoredRepo {
    path: String,
    name: String,
}

pub fn handle_commit_history(args: &Value, codebase: Option<&str>) -> Value {
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let author_filter = args.get("author").and_then(|v| v.as_str());
    let since_filter = match args.get("since").and_then(|v| v.as_str()) {
        Some(value) if !value.is_empty() => match parse_since_filter(value) {
            Ok(parsed) => Some(parsed),
            Err(error) => return json!({"error": error}),
        },
        _ => None,
    };
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
        .filter_map(|oid| {
            let oid = oid.ok()?;
            let commit = repo.find_commit(oid).ok()?;
            let author = commit.author().name().unwrap_or("").to_string();
            if let Some(filter) = author_filter {
                if !author
                    .to_ascii_lowercase()
                    .contains(&filter.to_ascii_lowercase())
                {
                    return None;
                }
            }
            let commit_time = commit.time().seconds();
            if let Some(since) = since_filter {
                if commit_time < since {
                    return None;
                }
            }
            Some(json!({
                "hash": oid.to_string()[..12].to_string(),
                "message": commit.summary().unwrap_or("").to_string(),
                "author": author,
                "time": commit_time,
            }))
        })
        .take(limit)
        .collect();

    json!({
        "commits": commits,
        "total": commits.len(),
        "limit": limit,
        "author": author_filter,
        "since": args.get("since").and_then(|v| v.as_str()),
    })
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

fn commit_search_repo_key(repo: &git2::Repository) -> String {
    repo.workdir()
        .or_else(|| repo.path().parent())
        .map(|path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
        .unwrap_or_else(|| repo.path().to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn commit_search_head_hash(repo: &git2::Repository) -> String {
    repo.head()
        .ok()
        .and_then(|head| head.target())
        .map(|oid| oid.to_string())
        .unwrap_or_default()
}

fn commit_search_result_cache_key(
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

fn commit_diff_context_score(
    repo: &git2::Repository,
    record: &CachedCommitRecord,
    query_lower: &str,
    query_tokens: &[String],
) -> f64 {
    if query_tokens.len() < 2 {
        return 0.0;
    }

    let subject = record.message.lines().next().unwrap_or("");
    if !commit_subject_needs_diff_context(subject) {
        return 0.0;
    }

    let context = record
        .diff_context
        .get_or_init(|| collect_commit_diff_context(repo, record.oid).unwrap_or_default());
    token_overlap_score_lower(
        query_lower,
        query_tokens,
        &context.text_lower,
        &context.tokens,
    )
}

fn collect_commit_diff_context(
    repo: &git2::Repository,
    oid: git2::Oid,
) -> Option<CommitDiffContext> {
    let commit = repo.find_commit(oid).ok()?;

    let Ok(tree) = commit.tree() else {
        return None;
    };
    let parent_tree = commit.parent(0).ok().and_then(|parent| parent.tree().ok());
    let Ok(diff) = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) else {
        return None;
    };

    let collector = std::cell::RefCell::new(CommitDiffContextCollector::default());
    let mut file_cb = |_delta: git2::DiffDelta<'_>, _progress: f32| true;
    let mut line_cb = |_delta: git2::DiffDelta<'_>,
                       _hunk: Option<git2::DiffHunk<'_>>,
                       line: git2::DiffLine<'_>| {
        if !matches!(line.origin(), '+' | '-') {
            return true;
        }
        if let Ok(content) = std::str::from_utf8(line.content()) {
            collector.borrow_mut().push_text(content);
        }
        true
    };

    let _ = diff.foreach(&mut file_cb, None, None, Some(&mut line_cb));

    let collector = collector.into_inner();
    if collector.tokens.is_empty() {
        None
    } else {
        Some(CommitDiffContext {
            text_lower: collector.tokens.join(" "),
            tokens: collector.tokens,
        })
    }
}

fn commit_subject_needs_diff_context(subject: &str) -> bool {
    let lowered = subject.trim().to_ascii_lowercase();
    lowered.starts_with("update ")
        || lowered.starts_with("docs update ")
        || lowered.starts_with("documentation update ")
}

#[derive(Default)]
struct CommitDiffContextCollector {
    seen: HashSet<String>,
    tokens: Vec<String>,
}

impl CommitDiffContextCollector {
    const TOKEN_LIMIT: usize = 48;

    fn push_text(&mut self, text: &str) {
        if self.tokens.len() >= Self::TOKEN_LIMIT {
            return;
        }

        for token in tokenize(text) {
            if is_commit_diff_context_stopword(&token) || !self.seen.insert(token.clone()) {
                continue;
            }
            self.tokens.push(token);
            if self.tokens.len() >= Self::TOKEN_LIMIT {
                break;
            }
        }
    }
}

fn is_commit_diff_context_stopword(token: &str) -> bool {
    matches!(
        token,
        "else"
            | "enum"
            | "false"
            | "from"
            | "impl"
            | "into"
            | "none"
            | "self"
            | "some"
            | "struct"
            | "true"
            | "use"
    )
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
    let normalized_path = normalize_repo_path(path);
    let is_git = git2::Repository::discover(&normalized_path).is_ok();
    registry.add(&normalized_path, name);
    json!({
        "registered": true,
        "path": normalized_path,
        "is_git": is_git,
        "hint": if is_git {
            "Run index(path) to build the graph and enable search for this repo."
        } else {
            "Registered a non-git directory. Index/search can still work, but git tools such as commit_history and commit_search will return errors until you target a git repository."
        }
    })
}

#[cfg_attr(not(test), allow(dead_code))]
fn handle_repo_remove(args: &Value, registry: &RepoRegistry) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() && name.is_empty() {
        return json!({"error": "Missing required parameter: path or name"});
    }
    if let Some((removed_path, removed_name)) = registry.remove_entry(
        (!path.is_empty()).then_some(path),
        (!name.is_empty()).then_some(name),
    ) {
        let mut response = json!({"removed": true, "path": removed_path, "name": removed_name});
        if !path.is_empty() {
            response.as_object_mut().unwrap().remove("name");
        } else {
            response.as_object_mut().unwrap().remove("path");
        }
        return response;
    }
    if !path.is_empty() {
        return json!({"removed": false, "path": path});
    }
    json!({"removed": false, "name": name})
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

#[cfg_attr(not(test), allow(dead_code))]
fn token_overlap_score(
    query: &str,
    query_tokens: &[String],
    document: &str,
    doc_tokens: &[String],
) -> f64 {
    let query_lower = query.to_ascii_lowercase();
    let document_lower = document.to_lowercase();
    token_overlap_score_lower(&query_lower, query_tokens, &document_lower, doc_tokens)
}

fn token_overlap_score_lower(
    query_lower: &str,
    query_tokens: &[String],
    document_lower: &str,
    doc_tokens: &[String],
) -> f64 {
    if query_tokens.is_empty() || doc_tokens.is_empty() {
        return 0.0;
    }

    let unique_doc_tokens: HashSet<&str> = doc_tokens.iter().map(|token| token.as_str()).collect();
    let token_qualities: Vec<f64> = query_tokens
        .iter()
        .map(|query_token| {
            doc_tokens
                .iter()
                .map(|doc_token| token_match_quality(query_token, doc_token))
                .fold(0.0, f64::max)
        })
        .collect();
    let matched_tokens = token_qualities
        .iter()
        .filter(|quality| **quality > 0.0)
        .count();
    if matched_tokens == 0 {
        return 0.0;
    }

    let exact_matches = query_tokens
        .iter()
        .filter(|query_token| unique_doc_tokens.contains(query_token.as_str()))
        .count();
    let doc_term_count = document_lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .count()
        .max(1);
    let coverage = matched_tokens as f64 / query_tokens.len() as f64;
    let exact_ratio = exact_matches as f64 / query_tokens.len() as f64;
    let quality_ratio = token_qualities.iter().sum::<f64>() / query_tokens.len() as f64;
    let density = matched_tokens as f64 / doc_term_count as f64;
    let phrase_bonus = if document_lower.contains(query_lower) {
        0.1
    } else {
        0.0
    };
    let starts_with_bonus = if document_lower.starts_with(query_lower) {
        0.1
    } else {
        0.0
    };

    (coverage * 0.3
        + exact_ratio * 0.15
        + quality_ratio * 0.2
        + density * 0.2
        + phrase_bonus
        + starts_with_bonus)
        .min(1.0)
}

fn token_match_quality(query_token: &str, doc_token: &str) -> f64 {
    if doc_token == query_token {
        1.0
    } else if doc_token.starts_with(query_token) {
        0.8
    } else if doc_token.ends_with(query_token) {
        0.65
    } else if doc_token.contains(query_token) {
        0.55
    } else if query_token.contains(doc_token) {
        0.4
    } else {
        0.0
    }
}

fn parse_since_filter(value: &str) -> Result<i64, String> {
    DateTime::parse_from_rfc3339(&format!("{value}T00:00:00Z"))
        .map(|date| date.with_timezone(&Utc).timestamp())
        .or_else(|_| {
            DateTime::parse_from_rfc3339(value).map(|date| date.with_timezone(&Utc).timestamp())
        })
        .map_err(|_| {
            format!(
                "Invalid since value: '{}'. Use RFC3339 or YYYY-MM-DD.",
                value
            )
        })
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

    #[test]
    fn test_repo_remove_accepts_name() {
        let path = temp_file("repos-remove.json");
        let repo_dir = std::env::temp_dir().join("contextro-repo-remove-name-test");
        let _ = std::fs::create_dir_all(&repo_dir);

        let registry = RepoRegistry::with_path(&path);
        assert!(registry.add(repo_dir.to_string_lossy().as_ref(), Some("repo-by-name")));

        let result = handle_repo_remove(&json!({"name":"repo-by-name"}), &registry);
        assert_eq!(result["removed"], true);
        assert_eq!(result["name"], "repo-by-name");
        assert!(registry.list().is_empty());

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir_all(repo_dir);
    }

    #[test]
    fn test_token_overlap_score_rewards_exact_phrase_and_density() {
        let exact = token_overlap_score(
            "fix reliability",
            &tokenize("fix reliability"),
            "fix reliability bug in session tracker",
            &tokenize("fix reliability bug in session tracker"),
        );
        let partial = token_overlap_score(
            "fix reliability",
            &tokenize("fix reliability"),
            "fix session tracker bug",
            &tokenize("fix session tracker bug"),
        );
        let diluted = token_overlap_score(
            "fix reliability",
            &tokenize("fix reliability"),
            "fix the repo registry and update changelog entries for release housekeeping",
            &tokenize(
                "fix the repo registry and update changelog entries for release housekeeping",
            ),
        );

        assert!(
            exact > partial,
            "exact phrase should outrank partial overlap"
        );
        assert!(
            partial > diluted,
            "denser partial match should outrank diluted overlap"
        );
    }

    #[test]
    fn test_token_overlap_score_prefers_prefix_subtoken_matches() {
        let bug_prefix = token_overlap_score(
            "fix bug",
            &tokenize("fix bug"),
            "add issue template bug_report",
            &tokenize("add issue template bug_report"),
        );
        let bug_suffix = token_overlap_score(
            "fix bug",
            &tokenize("fix bug"),
            "add issue template element_detection_bug",
            &tokenize("add issue template element_detection_bug"),
        );

        assert!(
            bug_prefix > bug_suffix,
            "prefix matches should outrank looser suffix-only matches for short queries"
        );
    }

    #[test]
    fn test_handle_commit_search_returns_differentiated_scores() {
        let repo_dir = temp_file("commit-search-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();

        let repo = git2::Repository::init(&repo_dir).unwrap();
        let signature = git2::Signature::now("Contextro Test", "test@example.com").unwrap();
        let mut parent: Option<git2::Oid> = None;

        for (idx, message) in [
            "chore release housekeeping",
            "fix session tracker bug",
            "fix reliability regression in session tracker",
        ]
        .iter()
        .enumerate()
        {
            std::fs::write(repo_dir.join("tracked.txt"), format!("commit-{idx}\n")).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("tracked.txt")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let parents = parent
                .map(|oid| vec![repo.find_commit(oid).unwrap()])
                .unwrap_or_default();
            let parent_refs = parents.iter().collect::<Vec<_>>();
            let oid = repo
                .commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    message,
                    &tree,
                    &parent_refs,
                )
                .unwrap();
            parent = Some(oid);
        }

        let result = handle_commit_search(
            &json!({"query":"fix reliability","limit":5}),
            Some(repo_dir.to_string_lossy().as_ref()),
        );
        let commits = result["commits"].as_array().expect("commits array");

        assert_eq!(
            commits[0]["message"],
            "fix reliability regression in session tracker"
        );
        assert!(commits[0]["score"].as_f64().unwrap() > commits[1]["score"].as_f64().unwrap());

        let _ = std::fs::remove_dir_all(repo_dir);
    }

    #[test]
    fn test_handle_commit_search_differentiates_single_token_release_queries() {
        let repo_dir = temp_file("commit-search-release-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();

        let repo = git2::Repository::init(&repo_dir).unwrap();
        let signature = git2::Signature::now("Contextro Test", "test@example.com").unwrap();
        let mut parent: Option<git2::Oid> = None;

        for (idx, message) in [
            "Release v1.6.3",
            "Update publication and release artifacts",
            "ci: add cargo publish job to release workflow",
        ]
        .iter()
        .enumerate()
        {
            std::fs::write(repo_dir.join("tracked.txt"), format!("commit-{idx}\n")).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("tracked.txt")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let parents = parent
                .map(|oid| vec![repo.find_commit(oid).unwrap()])
                .unwrap_or_default();
            let parent_refs = parents.iter().collect::<Vec<_>>();
            let oid = repo
                .commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    message,
                    &tree,
                    &parent_refs,
                )
                .unwrap();
            parent = Some(oid);
        }

        let result = handle_commit_search(
            &json!({"query":"release","limit":5}),
            Some(repo_dir.to_string_lossy().as_ref()),
        );
        let commits = result["commits"].as_array().expect("commits array");

        assert_eq!(commits[0]["message"], "Release v1.6.3");
        assert!(commits[0]["score"].as_f64().unwrap() > commits[1]["score"].as_f64().unwrap());
        assert!(commits[1]["score"].as_f64().unwrap() > commits[2]["score"].as_f64().unwrap());

        let _ = std::fs::remove_dir_all(repo_dir);
    }

    #[test]
    fn test_handle_commit_search_does_not_eagerly_expand_cache_on_initial_hit() {
        let repo_dir = temp_file("commit-search-cache-hit-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();

        let repo = git2::Repository::init(&repo_dir).unwrap();
        let signature = git2::Signature::now("Contextro Test", "test@example.com").unwrap();
        let mut parent: Option<git2::Oid> = None;

        for (idx, message) in [
            "fix reliability regression in session tracker",
            "chore release housekeeping",
            "docs update benchmark notes",
        ]
        .iter()
        .enumerate()
        {
            std::fs::write(repo_dir.join("tracked.txt"), format!("commit-{idx}\n")).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("tracked.txt")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let parents = parent
                .map(|oid| vec![repo.find_commit(oid).unwrap()])
                .unwrap_or_default();
            let parent_refs = parents.iter().collect::<Vec<_>>();
            let oid = repo
                .commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    message,
                    &tree,
                    &parent_refs,
                )
                .unwrap();
            parent = Some(oid);
        }

        let result = handle_commit_search(
            &json!({"query":"docs","limit":5}),
            Some(repo_dir.to_string_lossy().as_ref()),
        );

        assert_eq!(result["total"], 1);

        let initial_scan_limit = get_settings().read().commit_history_limit.max(500);
        let repo = git2::Repository::discover(&repo_dir).unwrap();
        let repo_key = commit_search_repo_key(&repo);
        let entry = commit_search_cache()
            .read()
            .get(&repo_key)
            .cloned()
            .expect("cache entry for repo");

        assert_eq!(entry.head_hash, commit_search_head_hash(&repo));
        assert_eq!(entry.scan_limit, initial_scan_limit);

        let _ = std::fs::remove_dir_all(repo_dir);
    }

    #[test]
    fn test_handle_commit_search_returns_cached_final_response() {
        let repo_dir = temp_file("commit-search-result-cache-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();

        let repo = git2::Repository::init(&repo_dir).unwrap();
        let signature = git2::Signature::now("Contextro Test", "test@example.com").unwrap();
        std::fs::write(repo_dir.join("tracked.txt"), "seed\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("tracked.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "initial commit",
            &tree,
            &[],
        )
        .unwrap();

        let repo_key = commit_search_repo_key(&repo);
        let head_hash = commit_search_head_hash(&repo);
        let cached = json!({
            "query": "release",
            "commits": [{
                "hash": "deadbeef1234",
                "message": "Release v9.9.9",
                "author": "Cache",
                "score": 1.0
            }],
            "total": 1
        });
        commit_search_result_cache().write().insert(
            commit_search_result_cache_key(&repo_key, &head_hash, "release", None, 5),
            cached.clone(),
        );

        let result = handle_commit_search(
            &json!({"query":"release","limit":5}),
            Some(repo_dir.to_string_lossy().as_ref()),
        );

        assert_eq!(result, cached);

        let _ = std::fs::remove_dir_all(repo_dir);
    }

    #[test]
    fn test_handle_commit_search_result_cache_invalidates_on_head_change() {
        let repo_dir = temp_file("commit-search-result-cache-head-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();

        let repo = git2::Repository::init(&repo_dir).unwrap();
        let signature = git2::Signature::now("Contextro Test", "test@example.com").unwrap();

        std::fs::write(repo_dir.join("tracked.txt"), "seed\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("tracked.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let base = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                "initial import",
                &tree,
                &[],
            )
            .unwrap();

        let before = handle_commit_search(
            &json!({"query":"release","limit":5}),
            Some(repo_dir.to_string_lossy().as_ref()),
        );
        assert_eq!(before["total"], 0);

        std::fs::write(repo_dir.join("tracked.txt"), "release\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("tracked.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.find_commit(base).unwrap();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "release benchmark cache",
            &tree,
            &[&parent],
        )
        .unwrap();

        let after = handle_commit_search(
            &json!({"query":"release","limit":5}),
            Some(repo_dir.to_string_lossy().as_ref()),
        );

        assert_eq!(after["total"], 1, "unexpected result: {after}");
        assert_eq!(after["commits"][0]["message"], "release benchmark cache");

        let _ = std::fs::remove_dir_all(repo_dir);
    }

    #[test]
    fn test_handle_commit_search_falls_back_beyond_initial_scan_limit() {
        let repo_dir = temp_file("commit-search-fallback-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();

        let repo = git2::Repository::init(&repo_dir).unwrap();
        let signature = git2::Signature::now("Contextro Test", "test@example.com").unwrap();
        let mut parent: Option<git2::Oid> = None;

        for (idx, message) in [
            "fix persistence regression in knowledge store",
            "knowledge persistence retrospective",
        ]
        .iter()
        .enumerate()
        {
            std::fs::write(repo_dir.join("tracked.txt"), format!("seed-{idx}\n")).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("tracked.txt")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let parents = parent
                .map(|oid| vec![repo.find_commit(oid).unwrap()])
                .unwrap_or_default();
            let parent_refs = parents.iter().collect::<Vec<_>>();
            let oid = repo
                .commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    message,
                    &tree,
                    &parent_refs,
                )
                .unwrap();
            parent = Some(oid);
        }

        for idx in 0..505 {
            std::fs::write(repo_dir.join("tracked.txt"), format!("filler-{idx}\n")).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("tracked.txt")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let parents = parent
                .map(|oid| vec![repo.find_commit(oid).unwrap()])
                .unwrap_or_default();
            let parent_refs = parents.iter().collect::<Vec<_>>();
            let oid = repo
                .commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    &format!("chore filler commit {idx}"),
                    &tree,
                    &parent_refs,
                )
                .unwrap();
            parent = Some(oid);
        }

        let fix_result = handle_commit_search(
            &json!({"query":"fix","limit":5}),
            Some(repo_dir.to_string_lossy().as_ref()),
        );
        let knowledge_result = handle_commit_search(
            &json!({"query":"knowledge","limit":5}),
            Some(repo_dir.to_string_lossy().as_ref()),
        );

        assert_eq!(fix_result["total"], 1);
        assert_eq!(
            fix_result["commits"][0]["message"],
            "fix persistence regression in knowledge store"
        );
        assert_eq!(knowledge_result["total"], 2);

        let _ = std::fs::remove_dir_all(repo_dir);
    }

    #[test]
    fn test_handle_commit_search_matches_terse_update_commits_via_diff_context() {
        let repo_dir = temp_file("commit-search-diff-context-repo");
        let bm25_path = repo_dir.join("crates/contextro-engines/src");
        std::fs::create_dir_all(&bm25_path).unwrap();

        let repo = git2::Repository::init(&repo_dir).unwrap();
        let signature = git2::Signature::now("Contextro Test", "test@example.com").unwrap();

        let file = bm25_path.join("bm25.rs");
        std::fs::write(&file, "pub fn build_query() { parse bm25 query terms }\n").unwrap();
        let mut index = repo.index().unwrap();
        index
            .add_path(Path::new("crates/contextro-engines/src/bm25.rs"))
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let base_commit = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                "initial import",
                &tree,
                &[],
            )
            .unwrap();

        std::fs::write(
            &file,
            "pub fn build_query() { query aware confidence scoring for bm25 tokens }\n",
        )
        .unwrap();
        let mut index = repo.index().unwrap();
        index
            .add_path(Path::new("crates/contextro-engines/src/bm25.rs"))
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.find_commit(base_commit).unwrap();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Update crates/contextro-engines/src/bm25.rs",
            &tree,
            &[&parent],
        )
        .unwrap();

        let result = handle_commit_search(
            &json!({"query":"query aware confidence scoring","limit":5}),
            Some(repo_dir.to_string_lossy().as_ref()),
        );

        assert_eq!(result["total"], 1, "unexpected result: {result}");
        assert_eq!(
            result["commits"][0]["message"],
            "Update crates/contextro-engines/src/bm25.rs"
        );

        let _ = std::fs::remove_dir_all(repo_dir);
    }

    #[test]
    fn test_handle_commit_history_applies_author_and_since_filters() {
        let repo_dir = temp_file("commit-history-filters-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();

        let repo = git2::Repository::init(&repo_dir).unwrap();
        let mut parent: Option<git2::Oid> = None;

        for (idx, (author, email, message)) in [
            ("Alice Example", "alice@example.com", "first commit"),
            ("Bob Example", "bob@example.com", "second commit"),
        ]
        .iter()
        .enumerate()
        {
            let signature = git2::Signature::now(author, email).unwrap();
            std::fs::write(repo_dir.join("tracked.txt"), format!("commit-{idx}\n")).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("tracked.txt")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let parents = parent
                .map(|oid| vec![repo.find_commit(oid).unwrap()])
                .unwrap_or_default();
            let parent_refs = parents.iter().collect::<Vec<_>>();
            let oid = repo
                .commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    message,
                    &tree,
                    &parent_refs,
                )
                .unwrap();
            parent = Some(oid);
        }

        let author_result = handle_commit_history(
            &json!({"author":"Bob Example","limit":10}),
            Some(repo_dir.to_string_lossy().as_ref()),
        );
        assert_eq!(author_result["total"], 1);
        assert_eq!(author_result["commits"][0]["author"], "Bob Example");

        let future_result = handle_commit_history(
            &json!({"since":"2999-01-01","limit":10}),
            Some(repo_dir.to_string_lossy().as_ref()),
        );
        assert_eq!(future_result["total"], 0);

        let _ = std::fs::remove_dir_all(repo_dir);
    }

    #[test]
    fn test_commit_search_filters_nonsense_queries_below_threshold() {
        let repo_dir = temp_file("commit-search-nonsense-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();

        let repo = git2::Repository::init(&repo_dir).unwrap();
        let signature = git2::Signature::now("Contextro Test", "test@example.com").unwrap();
        let mut parent: Option<git2::Oid> = None;

        for (idx, message) in ["Update LICENSE", "Update Dockerfile", "Update .gitignore"]
            .iter()
            .enumerate()
        {
            std::fs::write(repo_dir.join("tracked.txt"), format!("commit-{idx}\n")).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("tracked.txt")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let parents = parent
                .map(|oid| vec![repo.find_commit(oid).unwrap()])
                .unwrap_or_default();
            let parent_refs = parents.iter().collect::<Vec<_>>();
            let oid = repo
                .commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    message,
                    &tree,
                    &parent_refs,
                )
                .unwrap();
            parent = Some(oid);
        }

        let result = handle_commit_search(
            &json!({"query":"___definitely_not_a_real_commit_phrase___","limit":3}),
            Some(repo_dir.to_string_lossy().as_ref()),
        );

        assert_eq!(result["total"], 0);
        let _ = std::fs::remove_dir_all(repo_dir);
    }

    #[test]
    fn test_repo_add_reports_non_git_directory() {
        let path = temp_file("repo-add-non-git.json");
        let repo_dir = temp_file("repo-add-non-git-dir");
        std::fs::create_dir_all(&repo_dir).unwrap();

        let registry = RepoRegistry::with_path(&path);
        let result = handle_repo_add(
            &json!({"path": repo_dir.to_string_lossy().to_string()}),
            &registry,
        );

        assert_eq!(result["registered"], true);
        assert_eq!(result["is_git"], false);
        assert!(result["hint"]
            .as_str()
            .unwrap_or("")
            .contains("non-git directory"));

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir_all(repo_dir);
    }
}
