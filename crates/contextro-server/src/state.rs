//! Application state shared across all tool invocations.

use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time::Instant;

use parking_lot::RwLock;

use contextro_config::get_settings;
use contextro_core::graph::{
    RelationshipType, UniversalLocation, UniversalNode, UniversalRelationship,
};
use contextro_core::models::{Symbol, SymbolType};
use contextro_core::NodeType;
use contextro_engines::bm25::Bm25Engine;
use contextro_engines::cache::QueryCache;
use contextro_engines::graph::CodeGraph;
use contextro_engines::sandbox::OutputSandbox;
use contextro_memory::archive::CompactionArchive;
use contextro_memory::session::SessionTracker;
use contextro_memory::store::MemoryStore;
use contextro_tools::KnowledgeStore;
use contextro_tools::RepoRegistry;

pub struct AppState {
    pub started_at: Instant,
    pub graph: Arc<CodeGraph>,
    pub bm25: Arc<Bm25Engine>,
    pub query_cache: Arc<QueryCache>,
    pub sandbox: Arc<OutputSandbox>,
    pub session_tracker: Arc<SessionTracker>,
    pub memory_store: Arc<MemoryStore>,
    pub archive: Arc<CompactionArchive>,
    pub knowledge: Arc<KnowledgeStore>,
    pub repo_registry: Arc<RepoRegistry>,
    pub indexed: RwLock<bool>,
    pub codebase_path: RwLock<Option<String>>,
    pub chunk_count: AtomicUsize,
}

impl AppState {
    pub fn new() -> Self {
        let settings = get_settings().read();
        let storage_dir = &settings.storage_dir;
        let db_path = format!("{}/memories.db", storage_dir);
        std::fs::create_dir_all(storage_dir).ok();

        let memory_store = MemoryStore::new(&db_path)
            .unwrap_or_else(|_| MemoryStore::in_memory().expect("in-memory DB must work"));

        Self {
            started_at: Instant::now(),
            graph: Arc::new(CodeGraph::new()),
            bm25: Arc::new(Bm25Engine::new_in_memory()),
            query_cache: Arc::new(QueryCache::new(
                settings.search_cache_max_size,
                settings.search_cache_ttl_seconds,
            )),
            sandbox: Arc::new(OutputSandbox::new(
                settings.search_sandbox_max_entries,
                settings.search_sandbox_ttl_seconds,
            )),
            session_tracker: Arc::new(SessionTracker::default()),
            memory_store: Arc::new(memory_store),
            archive: Arc::new(CompactionArchive::new()),
            knowledge: Arc::new(KnowledgeStore::new()),
            repo_registry: Arc::new(RepoRegistry::new()),
            indexed: RwLock::new(false),
            codebase_path: RwLock::new(None),
            chunk_count: AtomicUsize::new(0),
        }
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
                line_count: sym.line_count(),
                docstring: if sym.docstring.is_empty() {
                    None
                } else {
                    Some(sym.docstring.clone())
                },
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
