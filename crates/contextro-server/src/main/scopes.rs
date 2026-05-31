use super::*;

use std::sync::Arc;

use contextro_engines::bm25::Bm25Engine;

use super::support::{
    normalize_repo_dir, process_repo_bm25, prune_process_repo_bm25, remember_process_repo_bm25,
    should_share_repo_bm25,
};

impl ContextroServer {
    pub(crate) fn handle_repo_remove(&self, args: &Value) -> Value {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if path.is_empty() && name.is_empty() {
            return json!({"error": "Missing required parameter: path or name"});
        }

        let removed = self.state.repo_registry.remove_entry(
            (!path.is_empty()).then_some(path),
            (!name.is_empty()).then_some(name),
        );
        let Some((removed_path, removed_name)) = removed else {
            return if !path.is_empty() {
                json!({"removed": false, "path": path})
            } else {
                json!({"removed": false, "name": name})
            };
        };

        self.prune_repo_scope_history(&removed_path);

        let removed_is_active = self
            .state
            .codebase_path
            .read()
            .clone()
            .map(|active| normalize_repo_dir(&active) == removed_path)
            .unwrap_or(false);

        let mut response = json!({
            "removed": true,
            "path": removed_path,
            "name": removed_name,
        });

        if !removed_is_active {
            self.state.persist_repo_scope_state();
            if !path.is_empty() {
                response.as_object_mut().unwrap().remove("name");
            } else {
                response.as_object_mut().unwrap().remove("path");
            }
            return response;
        }

        if let Some(previous_path) = self.take_previous_repo_scope_candidate() {
            *self.state.indexed.write() = false;
            *self.state.codebase_path.write() = None;
            let restore_result = self
                .try_restore_cached_repo_scope(&previous_path)
                .unwrap_or_else(|| {
                    self.handle_index_internal(&json!({"path": previous_path}), false)
                });
            if restore_result.get("status") == Some(&json!("done")) {
                response["active_scope_restored"] = json!(true);
                response["restored_path"] = self
                    .state
                    .codebase_path
                    .read()
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null);
                response["hint"] = json!(
                    "Removed repo was active, so Contextro restored the previous repo scope."
                );
                return response;
            }

            self.clear_active_scope();
            response["active_scope_cleared"] = json!(true);
            response["warning"] =
                json!("Removed repo was active and the previous scope could not be restored.");
            if let Some(error) = restore_result.get("error") {
                response["restore_error"] = error.clone();
            }
            response["hint"] =
                json!("Run index(path) or repo_add(path) to select a new active repo scope.");
            return response;
        }

        self.clear_active_scope();
        response["active_scope_cleared"] = json!(true);
        response["warning"] =
            json!("Removed repo was active and no previous repo scope was available.");
        response["hint"] =
            json!("Run index(path) or repo_add(path) to select a new active repo scope.");
        response
    }

    pub(crate) fn remember_repo_scope(&self, previous_path: String, next_path: String) {
        if previous_path == next_path {
            return;
        }

        let mut history = self.state.repo_scope_history.write();
        history.retain(|path| path != &next_path);
        if history.last() != Some(&previous_path) {
            history.push(previous_path);
        }
    }

    pub(crate) fn prune_repo_scope_history(&self, removed_path: &str) {
        self.state.prune_repo_snapshot(removed_path);
        prune_process_repo_bm25(removed_path);
        self.state
            .repo_scope_history
            .write()
            .retain(|path| path != removed_path);
    }

    pub(crate) fn take_previous_repo_scope_candidate(&self) -> Option<String> {
        let mut history = self.state.repo_scope_history.write();
        while let Some(candidate) = history.pop() {
            if std::path::Path::new(&candidate).is_dir() {
                return Some(candidate);
            }
        }
        None
    }

    pub(crate) fn repo_snapshot_matches_disk(&self, path: &str) -> bool {
        let settings = get_settings().read().clone();
        let current_hashes = contextro_indexing::discover_files_with_fingerprints(
            std::path::Path::new(path),
            &settings,
        )
        .fingerprints;
        let storage_dir = contextro_config::project_storage_dir(path);
        let stored_hashes = contextro_indexing::load_hashes(&storage_dir);
        if stored_hashes.is_empty() {
            return false;
        }

        let (added, modified, deleted) =
            contextro_indexing::diff_file_states(&current_hashes, &stored_hashes);
        added.is_empty() && modified.is_empty() && deleted.is_empty()
    }

    pub(crate) fn restore_repo_snapshot(
        &self,
        path: &str,
        snapshot: &RepoScopeSnapshot,
    ) -> (Value, RestoreSnapshotMetrics) {
        let restore_start = Instant::now();
        let graph_start = Instant::now();
        if snapshot.graph.is_empty() {
            self.state.graph.clear();
            self.state.build_graph(&snapshot.symbols);
            self.state.graph.compute_pagerank();
        } else {
            self.state.graph.restore_snapshot(&snapshot.graph);
        }
        let graph_ms = graph_start.elapsed().as_secs_f64() * 1000.0;

        let bm25_start = Instant::now();
        if let Some(cached_bm25) = self.state.repo_bm25(path) {
            self.state.replace_active_bm25(cached_bm25);
        } else if let Some(cached_bm25) = process_repo_bm25(path) {
            self.state.replace_active_bm25(cached_bm25.clone());
            self.state.remember_repo_bm25(path.to_string(), cached_bm25);
        } else {
            let next_bm25 = Arc::new(Bm25Engine::new_in_memory());
            next_bm25.index_chunks(&snapshot.chunks);
            self.state.replace_active_bm25(next_bm25.clone());
            self.state.remember_repo_bm25(path.to_string(), next_bm25);
            if should_share_repo_bm25(snapshot.chunks.len()) {
                remember_process_repo_bm25(path.to_string(), self.state.active_bm25());
            }
        }
        let bm25_ms = bm25_start.elapsed().as_secs_f64() * 1000.0;
        self.state
            .chunk_count
            .store(snapshot.chunks.len(), std::sync::atomic::Ordering::Relaxed);

        let vector_start = Instant::now();
        self.state.vector_index.clear();
        for chunk in &snapshot.chunks {
            if chunk.vector.is_empty() {
                continue;
            }
            self.state.vector_index.insert(
                chunk.vector.clone(),
                SearchResult {
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
        let vector_ms = vector_start.elapsed().as_secs_f64() * 1000.0;

        let scope_start = Instant::now();
        *self.state.indexed.write() = true;
        *self.state.codebase_path.write() = Some(path.to_string());
        self.state.invalidate_graph_views();
        self.state.knowledge.set_active_scope(Some(path));
        self.state.persist_repo_scope_state();
        let scope_ms = scope_start.elapsed().as_secs_f64() * 1000.0;

        let response = json!({
            "status": "done",
            "message": "Restored from cached repo snapshot.",
            "total_symbols": snapshot.symbols.len(),
            "total_chunks": snapshot.chunks.len(),
            "graph_nodes": self.state.graph.node_count(),
            "graph_relationships": self.state.graph.relationship_count(),
            "vector_chunks": self.state.vector_index.len(),
            "restored_from_cache": true,
        });

        (
            response,
            RestoreSnapshotMetrics {
                graph_ms,
                bm25_ms,
                vector_ms,
                scope_ms,
                total_ms: restore_start.elapsed().as_secs_f64() * 1000.0,
            },
        )
    }

    pub(crate) fn try_restore_cached_repo_scope(&self, path: &str) -> Option<Value> {
        let snapshot = self.load_valid_repo_snapshot(path)?;
        Some(self.restore_repo_snapshot(path, snapshot.as_ref()).0)
    }

    pub(crate) fn load_valid_repo_snapshot(&self, path: &str) -> Option<Arc<RepoScopeSnapshot>> {
        if let Some(snapshot) = self.state.repo_snapshot(path) {
            if !self.repo_snapshot_matches_disk(path) {
                self.state.prune_repo_snapshot(path);
                prune_process_repo_bm25(path);
                return None;
            }
            return Some(snapshot);
        }

        let snapshot = Arc::new(self.state.load_persisted_repo_snapshot(path)?);
        if !self.repo_snapshot_matches_disk(path) {
            self.state.prune_repo_snapshot(path);
            prune_process_repo_bm25(path);
            return None;
        }
        self.state
            .remember_repo_snapshot(path.to_string(), snapshot.clone());
        Some(snapshot)
    }

    pub(crate) fn load_repo_snapshot_if_hashes_match(
        &self,
        path: &str,
    ) -> Option<Arc<RepoScopeSnapshot>> {
        if let Some(snapshot) = self.state.repo_snapshot(path) {
            return Some(snapshot);
        }

        let snapshot = Arc::new(self.state.load_persisted_repo_snapshot(path)?);
        self.state
            .remember_repo_snapshot(path.to_string(), snapshot.clone());
        Some(snapshot)
    }

    pub(crate) fn clear_active_scope(&self) {
        self.state.graph.clear();
        self.state
            .replace_active_bm25(Arc::new(Bm25Engine::new_in_memory()));
        self.state.vector_index.clear();
        self.state.invalidate_graph_views();
        self.state
            .chunk_count
            .store(0, std::sync::atomic::Ordering::Relaxed);
        *self.state.indexed.write() = false;
        *self.state.codebase_path.write() = None;
        self.state.knowledge.set_active_scope(None);
        self.state.persist_repo_scope_state();
    }

    pub(crate) fn restore_persisted_active_scope(&self) {
        let Some(path) = self.state.codebase_path.read().clone() else {
            return;
        };

        if !std::path::Path::new(&path).is_dir() {
            self.clear_active_scope();
            return;
        }

        let result = self.handle_index_internal(&json!({"path": path}), false);
        if result.get("status") != Some(&json!("done")) {
            self.clear_active_scope();
        }
    }
}
