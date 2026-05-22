//! Application state shared across all tool invocations.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time::Instant;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use contextro_config::{get_settings, Settings};
use contextro_core::graph::{
    RelationshipType, UniversalLocation, UniversalNode, UniversalRelationship,
};
use contextro_core::models::{CodeChunk, Symbol, SymbolType};
use contextro_core::ContextroError;
use contextro_core::NodeType;
use contextro_engines::bm25::Bm25Engine;
use contextro_engines::cache::QueryCache;
use contextro_engines::graph::{CodeGraph, GraphSnapshot};
use contextro_engines::sandbox::OutputSandbox;
use contextro_engines::vector::VectorIndex;
use contextro_memory::archive::CompactionArchive;
use contextro_memory::session::SessionTracker;
use contextro_memory::store::MemoryStore;
use contextro_tools::KnowledgeStore;
use contextro_tools::RepoRegistry;

pub struct AppState {
    pub started_at: Instant,
    pub graph: Arc<CodeGraph>,
    pub bm25: Arc<Bm25Engine>,
    pub vector_index: Arc<VectorIndex>,
    pub query_cache: Arc<QueryCache>,
    #[allow(dead_code)]
    pub sandbox: Arc<OutputSandbox>,
    pub session_tracker: Arc<SessionTracker>,
    pub memory_store: Arc<MemoryStore>,
    pub archive: Arc<CompactionArchive>,
    pub knowledge: Arc<KnowledgeStore>,
    pub repo_registry: Arc<RepoRegistry>,
    pub repo_snapshots: RwLock<HashMap<String, RepoScopeSnapshot>>,
    pub repo_scope_history: RwLock<Vec<String>>,
    pub indexed: RwLock<bool>,
    pub codebase_path: RwLock<Option<String>>,
    pub chunk_count: AtomicUsize,
    repo_scope_state_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RepoScopeSnapshot {
    pub symbols: Vec<Symbol>,
    pub chunks: Vec<CodeChunk>,
    #[serde(default)]
    pub graph: GraphSnapshot,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct PersistedRepoScopeState {
    #[serde(default)]
    active_scope: Option<String>,
    #[serde(default)]
    history: Vec<String>,
}

impl AppState {
    pub fn new() -> Self {
        let settings = get_settings().read().clone();
        Self::from_settings(settings).expect("failed to initialize persistent app state storage")
    }

    pub(crate) fn from_settings(settings: Settings) -> Result<Self, ContextroError> {
        std::fs::create_dir_all(&settings.storage_dir)?;
        let storage_dir = PathBuf::from(&settings.storage_dir);
        let db_path = storage_dir.join("memories.db");
        let repo_scope_state_path = storage_dir.join("repo-scope.json");
        let persisted_repo_scope = load_repo_scope_state(&repo_scope_state_path);
        let memory_store = MemoryStore::new(db_path.to_string_lossy().as_ref())?;

        Ok(Self {
            started_at: Instant::now(),
            graph: Arc::new(CodeGraph::new()),
            bm25: Arc::new(Bm25Engine::new_in_memory()),
            vector_index: Arc::new(VectorIndex::new()),
            query_cache: Arc::new(QueryCache::new(
                settings.search_cache_max_size,
                settings.search_cache_ttl_seconds,
            )),
            sandbox: Arc::new(OutputSandbox::new(
                settings.search_sandbox_max_entries,
                settings.search_sandbox_ttl_seconds,
            )),
            session_tracker: Arc::new(SessionTracker::with_path(
                100,
                storage_dir.join("session-events.json"),
            )),
            memory_store: Arc::new(memory_store),
            archive: Arc::new(CompactionArchive::with_path(
                storage_dir.join("session-archive.json"),
                20,
                std::time::Duration::from_secs(86400),
            )),
            knowledge: Arc::new(KnowledgeStore::with_path(
                storage_dir.join("knowledge-store.json"),
            )),
            repo_registry: Arc::new(RepoRegistry::with_path(
                storage_dir.join("repo-registry.json"),
            )),
            repo_snapshots: RwLock::new(HashMap::new()),
            repo_scope_history: RwLock::new(
                persisted_repo_scope
                    .history
                    .into_iter()
                    .filter(|path| !path.is_empty())
                    .collect(),
            ),
            indexed: RwLock::new(false),
            codebase_path: RwLock::new(
                persisted_repo_scope
                    .active_scope
                    .filter(|path| !path.is_empty()),
            ),
            chunk_count: AtomicUsize::new(0),
            repo_scope_state_path,
        })
    }

    pub fn persist_repo_scope_state(&self) {
        let persisted = PersistedRepoScopeState {
            active_scope: self.codebase_path.read().clone(),
            history: self.repo_scope_history.read().clone(),
        };
        save_repo_scope_state(&self.repo_scope_state_path, &persisted);
    }

    pub fn remember_repo_snapshot(&self, path: String, snapshot: RepoScopeSnapshot) {
        self.repo_snapshots.write().insert(path, snapshot);
    }

    pub fn persist_repo_snapshot(&self, path: &str, snapshot: &RepoScopeSnapshot) {
        self.remember_repo_snapshot(path.to_string(), snapshot.clone());
        save_repo_snapshot(&repo_snapshot_path(path), snapshot);
    }

    pub fn repo_snapshot(&self, path: &str) -> Option<RepoScopeSnapshot> {
        self.repo_snapshots.read().get(path).cloned()
    }

    pub fn load_persisted_repo_snapshot(&self, path: &str) -> Option<RepoScopeSnapshot> {
        load_repo_snapshot(&repo_snapshot_path(path))
    }

    pub fn prune_repo_snapshot(&self, path: &str) {
        self.repo_snapshots.write().remove(path);
    }

    /// Build the code graph from parsed symbols.
    pub fn build_graph(&self, symbols: &[Symbol]) {
        let mut known: HashMap<String, String> = HashMap::new();

        for (i, sym) in symbols.iter().enumerate() {
            let node_id = format!("n{}", i);
            known.insert(sym.name.clone(), node_id.clone());

            let node = UniversalNode {
                id: node_id,
                name: sym.name.clone(),
                node_type: match sym.symbol_type {
                    SymbolType::Class => NodeType::Class,
                    SymbolType::Method => NodeType::Function,
                    SymbolType::Variable => NodeType::Variable,
                    SymbolType::Function => NodeType::Function,
                },
                location: UniversalLocation {
                    file_path: sym.filepath.clone(),
                    start_line: sym.line_start,
                    end_line: sym.line_end,
                    start_column: 0,
                    end_column: 0,
                    language: sym.language.clone(),
                },
                language: sym.language.clone(),
                content: if sym.code_snippet.is_empty() {
                    sym.signature.clone()
                } else {
                    sym.code_snippet.clone()
                },
                line_count: sym.line_count(),
                docstring: if sym.docstring.is_empty() {
                    None
                } else {
                    Some(sym.docstring.clone())
                },
                visibility: if sym.signature.trim_start().starts_with("pub ")
                    || sym.signature.trim_start().starts_with("pub(")
                {
                    "public".into()
                } else {
                    String::new()
                },
                is_async: sym.signature.contains("async fn"),
                parent: sym.parent.clone(),
                ..Default::default()
            };
            self.graph.add_node(node);
        }

        // Build call edges
        let mut rel_count = 0;
        for sym in symbols {
            let caller_id = match known.get(&sym.name) {
                Some(id) => id.clone(),
                None => continue,
            };
            for call in &sym.calls {
                if let Some(callee_id) = known.get(call) {
                    if &caller_id != callee_id {
                        self.graph.add_relationship(UniversalRelationship {
                            id: format!("r{}", rel_count),
                            source_id: caller_id.clone(),
                            target_id: callee_id.clone(),
                            relationship_type: RelationshipType::Calls,
                            strength: 1.0,
                        });
                        rel_count += 1;
                    }
                }
            }
        }
    }
}

fn load_repo_scope_state(path: &Path) -> PersistedRepoScopeState {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<PersistedRepoScopeState>(&bytes).ok())
        .unwrap_or_default()
}

fn repo_snapshot_path(path: &str) -> PathBuf {
    contextro_config::project_storage_dir(path).join("repo-snapshot.json")
}

fn load_repo_snapshot(path: &Path) -> Option<RepoScopeSnapshot> {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<RepoScopeSnapshot>(&bytes).ok())
}

fn save_repo_scope_state(path: &Path, state: &PersistedRepoScopeState) {
    if state.active_scope.is_none() && state.history.is_empty() {
        let _ = std::fs::remove_file(path);
        return;
    }

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp_path = path.with_extension("json.tmp");
    if let Ok(bytes) = serde_json::to_vec(state) {
        if std::fs::write(&tmp_path, bytes).is_ok() {
            let _ = std::fs::rename(&tmp_path, path);
        }
    }
}

fn save_repo_snapshot(path: &Path, snapshot: &RepoScopeSnapshot) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp_path = path.with_extension("json.tmp");
    if let Ok(bytes) = serde_json::to_vec(snapshot) {
        if std::fs::write(&tmp_path, bytes).is_ok() {
            let _ = std::fs::rename(&tmp_path, path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("contextro-state-{unique}-{name}"))
    }

    #[test]
    fn test_app_state_init_fails_instead_of_falling_back_to_in_memory_store() {
        let storage_path = temp_path("storage-file");
        fs::write(&storage_path, "not a directory").unwrap();

        let mut settings = Settings::default();
        settings.storage_dir = storage_path.to_string_lossy().to_string();

        let error = AppState::from_settings(settings)
            .err()
            .expect("storage init should fail");
        assert!(matches!(error, ContextroError::Io(_)));

        let _ = fs::remove_file(storage_path);
    }

    #[test]
    fn test_app_state_loads_and_clears_persisted_repo_scope_state() {
        let storage_dir = temp_path("storage-dir");
        fs::create_dir_all(&storage_dir).unwrap();

        let persisted_path = storage_dir.join("repo-scope.json");
        fs::write(
            &persisted_path,
            serde_json::to_vec(&PersistedRepoScopeState {
                active_scope: Some("/tmp/repo-b".into()),
                history: vec!["/tmp/repo-a".into()],
            })
            .unwrap(),
        )
        .unwrap();

        let mut settings = Settings::default();
        settings.storage_dir = storage_dir.to_string_lossy().to_string();
        let state = AppState::from_settings(settings).expect("state should load");

        assert_eq!(*state.codebase_path.read(), Some("/tmp/repo-b".into()));
        assert_eq!(
            &*state.repo_scope_history.read(),
            &vec!["/tmp/repo-a".to_string()]
        );

        *state.codebase_path.write() = None;
        state.repo_scope_history.write().clear();
        state.persist_repo_scope_state();

        assert!(!persisted_path.exists());

        let _ = fs::remove_dir_all(storage_dir);
    }

    #[test]
    fn test_repo_snapshot_round_trips_to_project_storage() {
        let storage_dir = temp_path("snapshot-storage-dir");
        fs::create_dir_all(&storage_dir).unwrap();

        let mut settings = Settings::default();
        settings.storage_dir = storage_dir.to_string_lossy().to_string();
        let state = AppState::from_settings(settings).expect("state should load");
        let repo_path = temp_path("snapshot-repo");
        let symbols = vec![Symbol {
            name: "snapshot_symbol".into(),
            symbol_type: SymbolType::Function,
            filepath: "src/lib.rs".into(),
            line_start: 1,
            line_end: 1,
            language: "rust".into(),
            signature: "pub fn snapshot_symbol()".into(),
            docstring: String::new(),
            parent: None,
            code_snippet: "pub fn snapshot_symbol() {}".into(),
            imports: vec![],
            calls: vec![],
        }];
        state.build_graph(&symbols);
        state.graph.compute_pagerank();

        let snapshot = RepoScopeSnapshot {
            symbols,
            chunks: vec![CodeChunk {
                id: "chunk-1".into(),
                text: "pub fn snapshot_symbol() {}".into(),
                filepath: "src/lib.rs".into(),
                symbol_name: "snapshot_symbol".into(),
                symbol_type: "function".into(),
                language: "rust".into(),
                line_start: 1,
                line_end: 1,
                signature: "pub fn snapshot_symbol()".into(),
                parent: String::new(),
                docstring: String::new(),
                vector: vec![0.1, 0.2, 0.3],
            }],
            graph: state.graph.snapshot(),
        };

        state.persist_repo_snapshot(repo_path.to_string_lossy().as_ref(), &snapshot);

        let restored = state
            .load_persisted_repo_snapshot(repo_path.to_string_lossy().as_ref())
            .expect("snapshot should load");

        assert_eq!(restored.symbols.len(), 1);
        assert_eq!(restored.symbols[0].name, "snapshot_symbol");
        assert_eq!(restored.chunks.len(), 1);
        assert_eq!(restored.chunks[0].vector, vec![0.1, 0.2, 0.3]);
        assert!(!restored.graph.is_empty());

        let _ = fs::remove_dir_all(storage_dir);
        let _ = fs::remove_dir_all(contextro_config::project_storage_dir(
            repo_path.to_string_lossy().as_ref(),
        ));
    }
}
