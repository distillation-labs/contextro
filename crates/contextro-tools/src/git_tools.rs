//! Git tools: commit_search, commit_history, diff_preview, repo_add, repo_remove, repo_status.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
#[cfg(test)]
use contextro_config::get_settings;
use contextro_git::{current_branch, head_hash};
use serde_json::{json, Value};

mod commit_search;

use commit_search::StoredRepo;

#[cfg(test)]
pub(crate) use commit_search::{
    commit_search_cache, commit_search_head_hash, commit_search_repo_key,
    commit_search_result_cache, commit_search_result_cache_key,
};
pub use commit_search::{handle_commit_search, prewarm_commit_search_cache, RepoRegistry};

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

pub fn handle_diff_preview(args: &Value, codebase: Option<&str>) -> Value {
    let repo_path = codebase.unwrap_or(".");
    let repo = match git2::Repository::discover(repo_path) {
        Ok(repo) => repo,
        Err(_) => return json!({"error": "Not a git repository"}),
    };

    let repo_root = match repo_root_path(&repo) {
        Some(root) => root,
        None => return json!({"error": "Failed to determine repository root"}),
    };
    let base = args.get("base").and_then(|value| value.as_str());
    let head = args.get("head").and_then(|value| value.as_str());
    let path = args
        .get("path")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty());
    let limit = args
        .get("limit")
        .and_then(|value| value.as_u64())
        .unwrap_or(20) as usize;
    let preview_lines = args
        .get("preview_lines")
        .and_then(|value| value.as_u64())
        .unwrap_or(4) as usize;

    let (diff, mode, base_label, head_label) =
        match build_diff_preview(&repo, &repo_root, base, head, path) {
            Ok(result) => result,
            Err(error) => return json!({"error": error}),
        };

    let stats = match diff.stats() {
        Ok(stats) => stats,
        Err(_) => return json!({"error": "Failed to collect diff stats"}),
    };
    let previews = match collect_diff_previews(&diff, &repo_root, preview_lines) {
        Ok(previews) => previews,
        Err(_) => return json!({"error": "Failed to read diff preview"}),
    };

    let candidate_total = previews.len();
    let truncated = candidate_total > limit;
    let files: Vec<Value> = previews
        .into_iter()
        .take(limit)
        .map(DiffFilePreview::into_json)
        .collect();
    let total = files.len();

    let repo_root_str = repo_root.to_string_lossy().to_string();
    let mut response = json!({
        "mode": mode,
        "base": base_label,
        "head": head_label,
        "path": path,
        "files": files,
        "total": total,
        "candidate_total": candidate_total,
        "truncated": truncated,
        "diffstat": {
            "files": stats.files_changed(),
            "insertions": stats.insertions(),
            "deletions": stats.deletions(),
        },
    });
    if let Some(branch) = current_branch(&repo_root_str) {
        response["branch"] = json!(branch);
    }
    if let Some(hash) = head_hash(&repo_root_str) {
        response["head_hash"] = json!(short_hash(&hash));
    }
    if candidate_total == 0 {
        response["hint"] = json!("No changes found for the selected diff.");
    }
    response
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

struct DiffFilePreview {
    path: String,
    status: &'static str,
    previous_path: Option<String>,
    insertions: usize,
    deletions: usize,
    preview: Vec<String>,
    preview_count: usize,
}

impl DiffFilePreview {
    fn from_delta(delta: &git2::DiffDelta<'_>, repo_root: &Path) -> Self {
        let previous_path = delta
            .old_file()
            .path()
            .map(|path| relativize_repo_path(path, repo_root));
        let current_path = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .map(|path| relativize_repo_path(path, repo_root))
            .unwrap_or_else(|| "<unknown>".to_string());
        Self {
            path: current_path,
            status: diff_status(delta.status()),
            previous_path,
            insertions: 0,
            deletions: 0,
            preview: Vec::new(),
            preview_count: 0,
        }
    }

    fn into_json(self) -> Value {
        let mut value = json!({
            "path": self.path,
            "status": self.status,
            "insertions": self.insertions,
            "deletions": self.deletions,
            "preview": self.preview,
        });
        if let Some(previous_path) = self.previous_path.filter(|old| old != &self.path) {
            value["previous_path"] = json!(previous_path);
        }
        value
    }
}

fn build_diff_preview<'repo>(
    repo: &'repo git2::Repository,
    repo_root: &Path,
    base: Option<&str>,
    head: Option<&str>,
    path: Option<&str>,
) -> Result<(git2::Diff<'repo>, &'static str, String, String), String> {
    let mut options = git2::DiffOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_typechange(true)
        .ignore_submodules(true);
    if let Some(path) = path {
        options.pathspec(normalize_diff_path(path, repo_root));
    }

    match head {
        Some(head_rev) => {
            let base_rev = base.unwrap_or("HEAD");
            let base_tree = resolve_tree(repo, base_rev)?;
            let head_tree = resolve_tree(repo, head_rev)?;
            let diff = repo
                .diff_tree_to_tree(Some(&base_tree), Some(&head_tree), Some(&mut options))
                .map_err(|_| format!("Failed to diff {}..{}", base_rev, head_rev))?;
            Ok((diff, "range", base_rev.to_string(), head_rev.to_string()))
        }
        None => {
            let base_rev = base.unwrap_or("HEAD");
            let base_tree = resolve_tree(repo, base_rev)?;
            let diff = repo
                .diff_tree_to_workdir_with_index(Some(&base_tree), Some(&mut options))
                .map_err(|_| format!("Failed to diff {} against the working tree", base_rev))?;
            Ok((
                diff,
                "worktree",
                base_rev.to_string(),
                "WORKTREE".to_string(),
            ))
        }
    }
}

fn collect_diff_previews(
    diff: &git2::Diff<'_>,
    repo_root: &Path,
    preview_lines: usize,
) -> Result<Vec<DiffFilePreview>, git2::Error> {
    let previews = RefCell::new(Vec::new());
    diff.foreach(
        &mut |delta, _| {
            previews
                .borrow_mut()
                .push(DiffFilePreview::from_delta(&delta, repo_root));
            true
        },
        None,
        None,
        Some(&mut |_, _, line| {
            if let Some(file) = previews.borrow_mut().last_mut() {
                match line.origin() {
                    '+' => file.insertions += 1,
                    '-' => file.deletions += 1,
                    _ => {}
                }
                if matches!(line.origin(), '+' | '-') && file.preview_count < preview_lines {
                    if let Some(compact) = compact_diff_line(line.origin(), line.content()) {
                        file.preview.push(compact);
                        file.preview_count += 1;
                    }
                }
            }
            true
        }),
    )?;
    Ok(previews.into_inner())
}

fn resolve_tree<'repo>(
    repo: &'repo git2::Repository,
    rev: &str,
) -> Result<git2::Tree<'repo>, String> {
    let object = repo
        .revparse_single(rev)
        .map_err(|_| format!("Unknown revision: {}", rev))?;
    object
        .peel_to_tree()
        .map_err(|_| format!("Revision '{}' does not resolve to a tree", rev))
}

fn repo_root_path(repo: &git2::Repository) -> Option<PathBuf> {
    repo.workdir()
        .map(Path::to_path_buf)
        .or_else(|| repo.path().parent().map(Path::to_path_buf))
}

fn normalize_diff_path(path: &str, repo_root: &Path) -> String {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        candidate
            .strip_prefix(repo_root)
            .unwrap_or(candidate)
            .to_string_lossy()
            .to_string()
    } else {
        path.to_string()
    }
}

fn relativize_repo_path(path: &Path, repo_root: &Path) -> String {
    path.strip_prefix(repo_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn diff_status(status: git2::Delta) -> &'static str {
    match status {
        git2::Delta::Added => "added",
        git2::Delta::Deleted => "deleted",
        git2::Delta::Modified => "modified",
        git2::Delta::Renamed => "renamed",
        git2::Delta::Copied => "copied",
        git2::Delta::Typechange => "typechange",
        git2::Delta::Untracked => "untracked",
        git2::Delta::Ignored => "ignored",
        git2::Delta::Unreadable => "unreadable",
        git2::Delta::Conflicted => "conflicted",
        _ => "unknown",
    }
}

fn compact_diff_line(origin: char, content: &[u8]) -> Option<String> {
    let line = String::from_utf8_lossy(content).trim().to_string();
    if line.is_empty() {
        return None;
    }
    let compact = if line.chars().count() > 120 {
        format!("{}...", line.chars().take(117).collect::<String>())
    } else {
        line
    };
    Some(format!("{origin}{compact}"))
}

fn short_hash(hash: &str) -> String {
    hash.chars().take(12).collect()
}

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
#[cfg(test)]
mod tests;
