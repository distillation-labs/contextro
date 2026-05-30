use super::*;

use std::sync::Arc;

use contextro_engines::bm25::Bm25Engine;

use super::support::{
    auto_populate_knowledge, maybe_prewarm_commit_search_cache, normalize_repo_dir,
    should_build_vector_index,
};

impl ContextroServer {
    pub(crate) fn handle_status(&self) -> Value {
        let uptime = self.state.started_at.elapsed().as_secs_f64();
        json!({
            "indexed": *self.state.indexed.read(),
            "codebase_path": *self.state.codebase_path.read(),
            "uptime_seconds": (uptime * 10.0).round() / 10.0,
            "graph_nodes": self.state.graph.node_count(),
            "graph_relationships": self.state.graph.relationship_count(),
            "cache_hit_rate": self.state.query_cache.hit_rate(),
            "memories": self.state.memory_store.count(),
        })
    }

    pub(crate) fn handle_health(&self) -> Value {
        json!({
            "status": "healthy",
            "uptime_seconds": (self.state.started_at.elapsed().as_secs_f64() * 10.0).round() / 10.0,
            "indexed": *self.state.indexed.read(),
            "version": env!("CARGO_PKG_VERSION"),
            "graph_nodes": self.state.graph.node_count(),
            "graph_relationships": self.state.graph.relationship_count(),
            "memories": self.state.memory_store.count(),
        })
    }

    pub(crate) fn handle_search(&self, args: &Value) -> Value {
        if !*self.state.indexed.read() || self.state.codebase_path.read().is_none() {
            return json!({
                "error": "No codebase loaded. Run 'index(path)' or 'repo_add(path)' to load an active repo scope."
            });
        }

        let codebase = self.state.codebase_path.read().clone();
        let bm25 = self.state.active_bm25();
        contextro_tools::search::handle_search_with_codebase(
            args,
            &bm25,
            &self.state.graph,
            &self.state.query_cache,
            &self.state.vector_index,
            codebase.as_deref(),
        )
    }

    pub(crate) fn handle_index(&self, args: &Value) -> Value {
        self.handle_index_internal(args, false)
    }

    pub(crate) fn handle_index_internal(&self, args: &Value, prewarm_commit_search: bool) -> Value {
        let request_start = Instant::now();
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        if path.is_empty() {
            return json!({"error": "Missing required parameter: path"});
        }
        if !std::path::Path::new(path).is_dir() {
            return json!({"error": format!("Not a directory: {}", path)});
        }

        let requested_path = normalize_repo_dir(path);
        let requested_path_ref = std::path::Path::new(&requested_path);
        let settings = get_settings().read().clone();
        let storage_dir = contextro_config::project_storage_dir(&requested_path);
        std::fs::create_dir_all(&storage_dir).ok();

        let pipeline = contextro_indexing::IndexingPipeline::new(settings.clone());

        // #1: Incremental re-indexing — check if files changed since last index
        let files = contextro_indexing::discover_files(requested_path_ref, &settings);
        let current_hashes = contextro_indexing::fingerprint_files(&files);
        let stored_hashes = contextro_indexing::load_hashes(&storage_dir);
        let (added, modified, deleted) =
            contextro_indexing::diff_file_states(&current_hashes, &stored_hashes);
        let changed_count = added.len() + modified.len() + deleted.len();
        let is_incremental = !stored_hashes.is_empty();
        let loaded_codebase = self.state.codebase_path.read().clone();

        // If nothing changed and we already have an index, skip re-parsing
        if Self::can_skip_reindex(
            &requested_path,
            loaded_codebase.as_deref(),
            *self.state.indexed.read(),
            is_incremental,
            changed_count,
        ) {
            return json!({
                "status": "done",
                "index_mode": "skipped",
                "message": "No files changed since last index.",
                "total_files": files.len(),
                "total_symbols": self.state.graph.node_count(),
                "total_chunks": self.state.chunk_count.load(std::sync::atomic::Ordering::Relaxed),
                "vector_chunks": self.state.vector_index.len(),
                "incremental": {"files_added": 0, "files_modified": 0, "files_deleted": 0, "files_unchanged": files.len()},
                "graph_nodes": self.state.graph.node_count(),
                "graph_relationships": self.state.graph.relationship_count(),
                "request_ms": round_ms(request_start.elapsed().as_secs_f64() * 1000.0),
            });
        }

        if is_incremental && changed_count == 0 {
            if let Some(snapshot) = self.load_repo_snapshot_if_hashes_match(&requested_path) {
                if let Some(previous_active) = loaded_codebase
                    .as_deref()
                    .map(normalize_repo_dir)
                    .filter(|previous_active| previous_active != &requested_path)
                {
                    self.remember_repo_scope(previous_active, requested_path.clone());
                }

                let (mut resp, restore_metrics) =
                    self.restore_repo_snapshot(&requested_path, &snapshot);
                resp["total_files"] = json!(files.len());
                resp["message"] = json!("Restored from persisted repo snapshot.");
                resp["index_mode"] = json!("restored");
                let knowledge_start = Instant::now();
                let kb_populated = auto_populate_knowledge(&requested_path, &self.state.knowledge);
                let knowledge_ms = knowledge_start.elapsed().as_secs_f64() * 1000.0;
                set_ms_field(&mut resp, "graph_ms", restore_metrics.graph_ms);
                set_ms_field(&mut resp, "bm25_ms", restore_metrics.bm25_ms);
                set_ms_field(&mut resp, "vector_ms", restore_metrics.vector_ms);
                set_ms_field(&mut resp, "scope_ms", restore_metrics.scope_ms);
                set_ms_field(&mut resp, "knowledge_ms", knowledge_ms);
                set_ms_field(&mut resp, "restore_ms", restore_metrics.total_ms);
                set_ms_field(
                    &mut resp,
                    "request_ms",
                    request_start.elapsed().as_secs_f64() * 1000.0,
                );
                if kb_populated > 0 {
                    resp["knowledge_docs_indexed"] = serde_json::json!(kb_populated);
                }
                return resp;
            }
        }

        match pipeline.index(requested_path_ref) {
            Ok((result, symbols, mut chunks)) => {
                let graph_start = Instant::now();
                self.state.graph.clear();
                self.state.build_graph(&symbols);
                self.state.graph.compute_pagerank();
                let graph_ms = graph_start.elapsed().as_secs_f64() * 1000.0;

                // Index chunks into the shared BM25 engine
                // Save hashes for next incremental run
                contextro_indexing::save_hashes(&current_hashes, &storage_dir);
                let bm25_start = Instant::now();
                let next_bm25 = Arc::new(Bm25Engine::new_in_memory());
                next_bm25.index_chunks(&chunks);
                self.state.replace_active_bm25(next_bm25.clone());
                self.state
                    .remember_repo_bm25(requested_path.clone(), next_bm25);
                let bm25_ms = bm25_start.elapsed().as_secs_f64() * 1000.0;
                self.state
                    .chunk_count
                    .store(chunks.len(), std::sync::atomic::Ordering::Relaxed);

                // Populate vector index
                let vector_start = Instant::now();
                self.state.vector_index.clear();
                let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
                if should_build_vector_index(texts.len()) {
                    if let Some(vectors) = contextro_indexing::embed_batch(&texts) {
                        for (chunk, vector) in chunks.iter_mut().zip(vectors) {
                            chunk.vector = vector.clone();
                            self.state.vector_index.insert(
                                vector,
                                contextro_core::models::SearchResult {
                                    id: chunk.id.clone(),
                                    filepath: chunk.filepath.clone(),
                                    symbol_name: chunk.symbol_name.clone(),
                                    symbol_type: chunk.symbol_type.clone(),
                                    language: chunk.language.clone(),
                                    line_start: chunk.line_start,
                                    line_end: chunk.line_end,
                                    score: 0.0,
                                    code: chunk.text.clone(),
                                    signature: chunk.signature.clone(),
                                    match_sources: vec!["vector".into()],
                                },
                            );
                        }
                    }
                }
                let vector_ms = vector_start.elapsed().as_secs_f64() * 1000.0;
                let total_chunks = chunks.len();

                let snapshot_start = Instant::now();
                self.state.persist_repo_snapshot(
                    &requested_path,
                    RepoScopeSnapshot {
                        symbols,
                        chunks,
                        graph: self.state.graph.snapshot(),
                    },
                );
                let snapshot_ms = snapshot_start.elapsed().as_secs_f64() * 1000.0;

                // Swap in the persistent BM25 engine
                if let Some(previous_active) = loaded_codebase
                    .as_deref()
                    .map(normalize_repo_dir)
                    .filter(|previous_active| previous_active != &requested_path)
                {
                    self.remember_repo_scope(previous_active, requested_path.clone());
                }

                let scope_start = Instant::now();
                *self.state.indexed.write() = true;
                *self.state.codebase_path.write() = Some(requested_path.clone());
                self.state.query_cache.invalidate();
                self.state.knowledge.set_active_scope(Some(&requested_path));
                self.state.persist_repo_scope_state();
                let scope_ms = scope_start.elapsed().as_secs_f64() * 1000.0;

                let prewarm_start = Instant::now();
                if prewarm_commit_search {
                    maybe_prewarm_commit_search_cache(&requested_path);
                }
                let prewarm_ms = prewarm_start.elapsed().as_secs_f64() * 1000.0;

                // Auto-populate knowledge base with project docs
                let knowledge_start = Instant::now();
                let kb_populated = auto_populate_knowledge(&requested_path, &self.state.knowledge);
                let knowledge_ms = knowledge_start.elapsed().as_secs_f64() * 1000.0;

                let mut resp = json!({
                    "status": "done",
                    "index_mode": "fresh",
                    "total_files": result.total_files,
                    "total_symbols": result.total_symbols,
                    "total_chunks": total_chunks,
                    "graph_nodes": self.state.graph.node_count(),
                    "graph_relationships": self.state.graph.relationship_count(),
                    "vector_chunks": self.state.vector_index.len(),
                    "time_seconds": (result.time_seconds * 100.0).round() / 100.0,
                    "discover_ms": round_ms(result.discover_ms),
                    "parse_ms": round_ms(result.parse_ms),
                    "chunk_ms": round_ms(result.chunk_ms),
                    "graph_ms": round_ms(graph_ms),
                    "bm25_ms": round_ms(bm25_ms),
                    "vector_ms": round_ms(vector_ms),
                    "snapshot_ms": round_ms(snapshot_ms),
                    "scope_ms": round_ms(scope_ms),
                    "prewarm_ms": round_ms(prewarm_ms),
                    "knowledge_ms": round_ms(knowledge_ms),
                    "request_ms": round_ms(request_start.elapsed().as_secs_f64() * 1000.0),
                });
                if is_incremental {
                    resp["incremental"] = json!({
                        "files_added": added.len(),
                        "files_modified": modified.len(),
                        "files_deleted": deleted.len(),
                        "files_unchanged": files.len() - changed_count,
                    });
                }
                if kb_populated > 0 {
                    resp["knowledge_docs_indexed"] = serde_json::json!(kb_populated);
                }
                resp
            }
            Err(e) => json!({"error": format!("Indexing failed: {}", e)}),
        }
    }
}
