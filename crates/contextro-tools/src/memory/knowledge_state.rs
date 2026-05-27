use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::knowledge_store::{KnowledgeDocument, PersistedKnowledgeState};

pub(crate) fn default_knowledge_scope() -> String {
    "__global__".to_string()
}

pub(crate) fn is_global_knowledge_scope(scope: &str) -> bool {
    scope.trim().is_empty() || scope == default_knowledge_scope()
}

pub(crate) fn normalize_knowledge_scope(scope: Option<&str>) -> String {
    let scope = scope.unwrap_or("").trim();
    if scope.is_empty() {
        return default_knowledge_scope();
    }
    std::fs::canonicalize(scope)
        .unwrap_or_else(|_| PathBuf::from(scope))
        .to_string_lossy()
        .to_string()
}

pub(crate) fn active_docs_mut(
    state: &mut PersistedKnowledgeState,
) -> &mut HashMap<String, KnowledgeDocument> {
    let scope = state.active_scope.clone();
    state.scopes.entry(scope).or_default()
}

pub(crate) fn active_docs(
    state: &PersistedKnowledgeState,
) -> Option<&HashMap<String, KnowledgeDocument>> {
    state.scopes.get(&state.active_scope)
}

pub(crate) fn load_knowledge_state(path: &Path) -> PersistedKnowledgeState {
    let Ok(bytes) = std::fs::read(path) else {
        return PersistedKnowledgeState::default();
    };

    let raw = serde_json::from_slice::<Value>(&bytes).ok();
    let state = serde_json::from_slice::<PersistedKnowledgeState>(&bytes).unwrap_or_default();
    normalize_loaded_knowledge_state(state, raw.as_ref())
}

fn normalize_loaded_knowledge_state(
    mut state: PersistedKnowledgeState,
    raw: Option<&Value>,
) -> PersistedKnowledgeState {
    migrate_legacy_global_scope(&mut state);

    let active_scope_missing_or_blank = raw_active_scope_missing_or_blank(raw);

    if state.active_scope.trim().is_empty() {
        state.active_scope = default_knowledge_scope();
    }

    let should_promote_repo_scope = is_global_knowledge_scope(&state.active_scope)
        && (!state.scopes.contains_key(&state.active_scope) || active_scope_missing_or_blank);
    if should_promote_repo_scope {
        if let Some(scope) = sole_repo_knowledge_scope(&state.scopes) {
            state.active_scope = scope;
        }
    }

    state
}

fn raw_active_scope_missing_or_blank(raw: Option<&Value>) -> bool {
    let Some(raw) = raw else {
        return false;
    };

    let Some(object) = raw.as_object() else {
        return false;
    };

    match object.get("active_scope") {
        None => true,
        Some(Value::String(scope)) => scope.trim().is_empty(),
        Some(Value::Null) => true,
        _ => false,
    }
}

fn migrate_legacy_global_scope(state: &mut PersistedKnowledgeState) {
    let Some(legacy_docs) = state.scopes.remove("") else {
        return;
    };

    let global_scope = default_knowledge_scope();
    let docs = state.scopes.entry(global_scope).or_default();
    for (name, document) in legacy_docs {
        docs.entry(name).or_insert(document);
    }
}

fn sole_repo_knowledge_scope(
    scopes: &HashMap<String, HashMap<String, KnowledgeDocument>>,
) -> Option<String> {
    let mut repo_scopes: Vec<String> = scopes
        .iter()
        .filter(|(scope, docs)| !docs.is_empty() && !is_global_knowledge_scope(scope))
        .map(|(scope, _)| scope.clone())
        .collect();

    if repo_scopes.len() == 1 {
        repo_scopes.pop()
    } else {
        None
    }
}

pub(crate) fn read_knowledge_source(path: &Path) -> String {
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

pub(crate) fn canonicalize_if_exists(path: &Path) -> Option<PathBuf> {
    if !path.exists() {
        return None;
    }
    Some(std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
}

pub(crate) fn knowledge_metadata_text(name: &str, source_path: Option<&Path>) -> String {
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

fn normalize_knowledge_term(term: &str) -> String {
    let term = term.trim().to_lowercase();
    if term.len() <= 4 {
        return term;
    }
    if let Some(stem) = term.strip_suffix("ies") {
        return format!("{stem}y");
    }
    if !term.ends_with("ss") {
        if let Some(stem) = term.strip_suffix('s') {
            return stem.to_string();
        }
    }
    term
}

pub(crate) fn knowledge_terms(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|term| term.len() >= 3)
        .map(normalize_knowledge_term)
        .collect()
}

pub(crate) fn knowledge_terms_match(doc_terms: &[String], query_term: &str) -> bool {
    doc_terms
        .iter()
        .any(|doc_term| doc_term == query_term || doc_term.contains(query_term))
}

pub(crate) fn summarize_preview(text: &str, max_chars: usize) -> Option<String> {
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

pub(crate) fn truncate_text(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}
