use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use contextro_config::get_settings;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use super::knowledge_state::{
    active_docs, active_docs_mut, default_knowledge_scope, knowledge_metadata_text,
    knowledge_terms, knowledge_terms_match, load_knowledge_state, normalize_knowledge_scope,
    summarize_preview,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct KnowledgeChunk {
    content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct KnowledgeDocument {
    chunks: Vec<KnowledgeChunk>,
    metadata_text: String,
    source_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct PersistedKnowledgeState {
    #[serde(default = "default_knowledge_scope")]
    pub(crate) active_scope: String,
    #[serde(default)]
    pub(crate) scopes: HashMap<String, HashMap<String, KnowledgeDocument>>,
}

impl Default for PersistedKnowledgeState {
    fn default() -> Self {
        Self {
            active_scope: default_knowledge_scope(),
            scopes: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnowledgeDocSummary {
    pub name: String,
    pub chunks: usize,
    pub preview: Option<String>,
    pub source_path: Option<String>,
}

/// Knowledge base: lightweight in-memory doc store with metadata-aware search.
pub struct KnowledgeStore {
    state: RwLock<PersistedKnowledgeState>,
    file_path: PathBuf,
}

impl KnowledgeStore {
    pub fn new() -> Self {
        let storage_dir = get_settings().read().storage_dir.clone();
        Self::with_path(PathBuf::from(storage_dir).join("knowledge-store.json"))
    }

    pub fn with_path<P: Into<PathBuf>>(file_path: P) -> Self {
        let file_path = file_path.into();
        Self {
            state: RwLock::new(load_knowledge_state(&file_path)),
            file_path,
        }
    }

    pub fn set_active_scope(&self, scope: Option<&str>) {
        let scope = normalize_knowledge_scope(scope);
        let mut state = self.state.write();
        if state.active_scope == scope {
            return;
        }
        state.active_scope = scope;
        self.save_locked(&state);
    }

    /// Index content under `name`. Returns the number of chunks stored.
    pub fn add(&self, name: &str, content: &str, source_path: Option<&Path>) -> usize {
        let Some(document) = build_knowledge_document(name, content, source_path) else {
            return 0;
        };
        let count = document.chunks.len();
        let mut state = self.state.write();
        let docs = active_docs_mut(&mut state);
        docs.insert(name.to_string(), document);
        self.save_locked(&state);
        count
    }

    /// Index multiple documents under the active scope with a single state write.
    pub fn add_documents<I>(&self, docs: I) -> usize
    where
        I: IntoIterator<Item = (String, String, Option<PathBuf>)>,
    {
        let prepared: Vec<(String, KnowledgeDocument)> = docs
            .into_iter()
            .filter_map(|(name, content, source_path)| {
                let document = build_knowledge_document(&name, &content, source_path.as_deref())?;
                Some((name, document))
            })
            .collect();
        if prepared.is_empty() {
            return 0;
        }

        let count = prepared.len();
        let mut state = self.state.write();
        let active_docs = active_docs_mut(&mut state);
        for (name, document) in prepared {
            active_docs.insert(name, document);
        }
        self.save_locked(&state);
        count
    }

    pub fn contains(&self, name: &str) -> bool {
        let state = self.state.read();
        active_docs(&state)
            .map(|docs| docs.contains_key(name))
            .unwrap_or(false)
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<(String, String)> {
        let query_lower = query.to_lowercase();
        // Split into meaningful words (3+ chars) for fallback word matching
        let words: Vec<&str> = query_lower
            .split_whitespace()
            .filter(|w| w.len() >= 3)
            .collect();
        let normalized_query_terms = knowledge_terms(query);

        let state = self.state.read();
        let mut results: Vec<(String, String, usize)> = Vec::new();

        let Some(docs) = active_docs(&state) else {
            return Vec::new();
        };

        for (name, doc) in docs {
            let metadata_lower = doc.metadata_text.to_lowercase();
            for chunk in &doc.chunks {
                let chunk_lower = chunk.content.to_lowercase();

                // Search metadata (name/path aliases) and content together, but only
                // return the original chunk content so results stay truthful.
                let mut score = 0usize;

                if metadata_lower.contains(&query_lower) {
                    score += 120;
                }
                if chunk_lower.contains(&query_lower) {
                    score += 100;
                }
                if score == 0 && !words.is_empty() {
                    let metadata_matches = words
                        .iter()
                        .filter(|word| metadata_lower.contains(*word))
                        .count();
                    let content_matches = words
                        .iter()
                        .filter(|word| chunk_lower.contains(*word))
                        .count();
                    score = metadata_matches * 12 + content_matches * 4;
                }
                if score == 0 && !normalized_query_terms.is_empty() {
                    let metadata_terms = knowledge_terms(&metadata_lower);
                    let content_terms = knowledge_terms(&chunk_lower);
                    let metadata_matches = normalized_query_terms
                        .iter()
                        .filter(|term| knowledge_terms_match(&metadata_terms, term))
                        .count();
                    let content_matches = normalized_query_terms
                        .iter()
                        .filter(|term| knowledge_terms_match(&content_terms, term))
                        .count();
                    score = metadata_matches * 10 + content_matches * 3;
                }

                if score >= 3 {
                    results.push((name.clone(), chunk.content.clone(), score));
                }
            }
        }

        results.sort_by_key(|result| Reverse(result.2));
        results
            .into_iter()
            .take(limit)
            .map(|(name, chunk, _)| (name, chunk))
            .collect()
    }

    pub fn show(&self) -> Vec<KnowledgeDocSummary> {
        let state = self.state.read();
        let Some(docs) = active_docs(&state) else {
            return Vec::new();
        };

        let mut summaries: Vec<KnowledgeDocSummary> = docs
            .iter()
            .map(|(name, doc)| KnowledgeDocSummary {
                name: name.clone(),
                chunks: doc.chunks.len(),
                preview: doc
                    .chunks
                    .first()
                    .and_then(|chunk| summarize_preview(&chunk.content, 120)),
                source_path: doc.source_path.clone(),
            })
            .collect();
        summaries.sort_by(|left, right| left.name.cmp(&right.name));
        summaries
    }

    pub fn remove(&self, name: &str) -> bool {
        let mut state = self.state.write();
        let scope = state.active_scope.clone();
        let mut removed = false;
        let mut scope_empty = false;
        if let Some(docs) = state.scopes.get_mut(&scope) {
            removed = docs.remove(name).is_some();
            scope_empty = docs.is_empty();
        }
        if removed {
            if scope_empty {
                state.scopes.remove(&scope);
            }
            self.save_locked(&state);
        }
        removed
    }

    pub fn clear(&self) -> usize {
        let mut state = self.state.write();
        let scope = state.active_scope.clone();
        let removed = state
            .scopes
            .remove(&scope)
            .map(|docs| docs.len())
            .unwrap_or(0);
        if removed > 0 {
            self.save_locked(&state);
        }
        removed
    }

    fn save_locked(&self, state: &PersistedKnowledgeState) {
        if let Some(parent) = self.file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp_path = self.file_path.with_extension("json.tmp");
        if let Ok(bytes) = serde_json::to_vec(state) {
            if std::fs::write(&tmp_path, bytes).is_ok() {
                let _ = std::fs::rename(&tmp_path, &self.file_path);
            }
        }
    }
}

impl Default for KnowledgeStore {
    fn default() -> Self {
        Self::new()
    }
}

fn build_knowledge_document(
    name: &str,
    content: &str,
    source_path: Option<&Path>,
) -> Option<KnowledgeDocument> {
    if content.trim().is_empty() {
        return None;
    }
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return None;
    }

    Some(KnowledgeDocument {
        chunks: lines
            .chunks(20)
            .map(|chunk_lines| KnowledgeChunk {
                content: chunk_lines.join("\n"),
            })
            .collect(),
        metadata_text: knowledge_metadata_text(name, source_path),
        source_path: source_path.map(|path| path.to_string_lossy().to_string()),
    })
}
