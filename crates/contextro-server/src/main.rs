//! Contextro MCP server binary — single compiled Rust binary.

use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use contextro_core::models::SearchResult;
use rmcp::model::*;
use rmcp::Error as McpError;
use rmcp::{ServerHandler, ServiceExt};
use serde_json::{json, Value};
use tracing::info;

use contextro_config::get_settings;

#[path = "http.rs"]
mod http;
#[path = "state.rs"]
mod state;
#[path = "tool_registry.rs"]
mod tool_registry;
#[path = "update_check.rs"]
mod update_check;
use state::{AppState, RepoScopeSnapshot};

/// The Contextro MCP server.
#[derive(Clone)]
pub struct ContextroServer {
    state: Arc<AppState>,
}

#[derive(Debug, Default)]
struct RestoreSnapshotMetrics {
    graph_ms: f64,
    bm25_ms: f64,
    vector_ms: f64,
    scope_ms: f64,
    total_ms: f64,
}

fn round_ms(ms: f64) -> f64 {
    (ms * 10.0).round() / 10.0
}

fn set_ms_field(response: &mut Value, key: &str, ms: f64) {
    response[key] = json!(round_ms(ms));
}

impl ContextroServer {
    pub fn new() -> Self {
        Self::from_state(AppState::new())
    }

    fn from_state(state: AppState) -> Self {
        let server = Self {
            state: Arc::new(state),
        };
        server.restore_persisted_active_scope();
        server
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn with_settings(settings: contextro_config::Settings) -> Self {
        let state = AppState::from_settings(settings).expect("failed to initialize app state");
        Self::from_state(state)
    }

    fn can_skip_reindex(
        requested_path: &str,
        loaded_path: Option<&str>,
        indexed: bool,
        is_incremental: bool,
        changed_count: usize,
    ) -> bool {
        if !indexed || !is_incremental || changed_count != 0 {
            return false;
        }

        let Some(loaded_path) = loaded_path else {
            return false;
        };

        normalize_repo_dir(requested_path) == normalize_repo_dir(loaded_path)
    }

    pub(crate) fn dispatch(&self, name: &str, args: Value) -> CallToolResult {
        let s = &self.state;
        let codebase = s.codebase_path.read().clone();
        let cb = codebase.as_deref();
        let cache_args = strip_render_only_args(&args);
        let sanitized_args = sanitize_tool_args(&args, cb);
        let tracked_args = sanitized_args
            .as_object()
            .filter(|map| !map.is_empty())
            .map(|_| sanitized_args.clone());
        let summary = summarize_tool_call(name, &sanitized_args, None);
        let response_cache_key = response_cache_key(name, &cache_args, cb);

        s.session_tracker
            .track(name, &summary, tracked_args.clone());

        if let Some(cache_key) = response_cache_key.as_deref() {
            if let Some(cached) = s.query_cache.get(cache_key) {
                return cached_tool_result(cached, &args);
            }
        }

        let result = match name {
            "status" => self.handle_status(),
            "health" => self.handle_health(),
            "index" => self.handle_index(&args),
            "search" => self.handle_search(&args),
            "find_symbol" => self.handle_find_symbol(&args),
            "find_callers" => {
                contextro_tools::graph_tools::handle_find_callers(&args, &s.graph, cb)
            }
            "find_callees" => {
                contextro_tools::graph_tools::handle_find_callees(&args, &s.graph, cb)
            }
            "explain" => contextro_tools::graph_tools::handle_explain(&args, &s.graph, cb),
            "impact" => contextro_tools::graph_tools::handle_impact(&args, &s.graph, cb),
            "overview" => contextro_tools::analysis::handle_overview(
                &s.graph,
                cb,
                s.chunk_count.load(std::sync::atomic::Ordering::Relaxed),
                s.vector_index.len(),
            ),
            "architecture" => contextro_tools::analysis::handle_architecture(&args, &s.graph, cb),
            "analyze" => contextro_tools::analysis::handle_analyze(&args, &s.graph, cb),
            "focus" => contextro_tools::analysis::handle_focus(&args, &s.graph, cb),
            "dead_code" => contextro_tools::analysis::handle_dead_code(&args, &s.graph, cb),
            "circular_dependencies" => {
                contextro_tools::analysis::handle_circular_dependencies(&s.graph, cb)
            }
            "test_coverage_map" => {
                contextro_tools::analysis::handle_test_coverage_map(&s.graph, cb)
            }
            "remember" => contextro_tools::memory::handle_remember(&args, &s.memory_store),
            "recall" => contextro_tools::memory::handle_recall(&args, &s.memory_store),
            "forget" => contextro_tools::memory::handle_forget(&args, &s.memory_store),
            "tags" => contextro_tools::memory::handle_tags(&s.memory_store),
            "knowledge" => contextro_tools::memory::handle_knowledge(&args, &s.knowledge),
            "compact" => contextro_tools::session::handle_compact(&args, &s.archive),
            "session_snapshot" => {
                contextro_tools::session::handle_session_snapshot(&args, &s.session_tracker)
            }
            "restore" => contextro_tools::session::handle_restore(
                cb,
                *s.indexed.read(),
                s.graph.node_count(),
                s.graph.relationship_count(),
            ),
            "retrieve" => contextro_tools::session::handle_retrieve(&args, &s.archive),
            "commit_search" => contextro_tools::git_tools::handle_commit_search(&args, cb),
            "commit_history" => contextro_tools::git_tools::handle_commit_history(&args, cb),
            "repo_add" => {
                let reg_result =
                    contextro_tools::git_tools::handle_repo_add(&args, &s.repo_registry);
                if reg_result.get("error").is_some() {
                    reg_result
                } else {
                    // Auto-index the added repo
                    let index_result = self.handle_index_internal(&args, false);
                    let mut combined = reg_result;
                    if index_result.get("status") == Some(&json!("done")) {
                        combined["indexed"] = json!(true);
                        combined["graph_nodes"] = index_result["graph_nodes"].clone();
                        combined["graph_relationships"] =
                            index_result["graph_relationships"].clone();
                        combined["total_symbols"] = index_result["total_symbols"].clone();
                        if combined.get("hint")
                            == Some(&json!(
                                "Run index(path) to build the graph and enable search for this repo."
                            ))
                        {
                            combined["hint"] = json!(
                                "Repository registered, indexed, and set as the active repo scope."
                            );
                        }
                    } else if let Some(error) = index_result.get("error") {
                        combined["indexed"] = json!(false);
                        combined["index_error"] = error.clone();
                    }
                    combined
                }
            }
            "repo_remove" => self.handle_repo_remove(&args),
            "repo_status" => contextro_tools::git_tools::handle_repo_status(&s.repo_registry),
            "code" => contextro_tools::code::handle_code(&args, &s.graph, cb),
            "audit" => contextro_tools::artifacts::handle_audit(&s.graph, cb),
            "docs_bundle" => contextro_tools::artifacts::handle_docs_bundle(&args, &s.graph, cb),
            "sidecar_export" => {
                contextro_tools::artifacts::handle_sidecar_export(&args, &s.graph, cb)
            }
            "skill_prompt" => contextro_tools::artifacts::handle_skill_prompt(),
            "introspect" => contextro_tools::artifacts::handle_introspect(&args),
            "refactor_check" => self.handle_refactor_check(&args),
            "completion_check" => self.handle_completion_check(&args),
            _ => {
                json!({"error": format!("Unknown tool: '{}'. Use introspect() to find the right tool.", name)})
            }
        };

        // ── Response optimization (#1, #5, #7, #9) ──────────────────────────
        let max_tokens = args.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

        // #5: Strip absolute codebase prefix from all file paths in response
        let result = if let Some(base) = cb {
            if response_needs_path_stripping(&result, base) {
                strip_response_paths(result, base)
            } else {
                result
            }
        } else {
            result
        };

        if let Some(cache_key) = response_cache_key.as_deref() {
            if result.get("error").is_none() {
                s.query_cache.put(cache_key, result.clone());
            }
        }

        if result.get("error").is_some() {
            // #8: Actionable errors — add fuzzy suggestions for symbol-not-found
            let err_text = result["error"].as_str().unwrap_or("");
            let enhanced = if err_text.contains("not found")
                && matches!(
                    name,
                    "find_symbol"
                        | "find_callers"
                        | "find_callees"
                        | "explain"
                        | "impact"
                        | "refactor_check"
                        | "completion_check"
                ) {
                if let Some(sym) = err_text.split('\'').nth(1) {
                    // Try fuzzy graph search first, then edit distance
                    let mut suggestions = s.graph.find_nodes_by_name(sym, false);
                    if suggestions.is_empty() {
                        // Edit distance fallback: find symbols within distance 2
                        let all = s.graph.find_nodes_by_name("", false);
                        let sym_lower = sym.to_lowercase();
                        suggestions = all
                            .into_iter()
                            .filter(|n| {
                                let name_lower = n.name.to_lowercase();
                                edit_distance(&sym_lower, &name_lower) <= 2
                                    || name_lower.contains(&sym_lower)
                                    || sym_lower.contains(&name_lower)
                            })
                            .collect();
                    }
                    if !suggestions.is_empty() {
                        let sugg: Vec<String> = suggestions
                            .iter()
                            .take(3)
                            .map(|n| {
                                format!(
                                    "{} ({}:{})",
                                    n.name,
                                    strip_codebase(&n.location.file_path, cb),
                                    n.location.start_line
                                )
                            })
                            .collect();
                        json!({"error": err_text, "did_you_mean": sugg, "hint": format!("Try: find_symbol(symbol_name=\"{}\", exact=false)", take_chars(sym, 4))})
                    } else {
                        result
                    }
                } else {
                    result
                }
            } else {
                result
            };
            CallToolResult::error(vec![Content::text(format_response(&enhanced, max_tokens))])
        } else {
            CallToolResult::success(vec![Content::text(format_response(&result, max_tokens))])
        }
    }

    fn handle_status(&self) -> Value {
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

    fn handle_health(&self) -> Value {
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

    fn handle_search(&self, args: &Value) -> Value {
        if !*self.state.indexed.read() || self.state.codebase_path.read().is_none() {
            return json!({
                "error": "No codebase loaded. Run 'index(path)' or 'repo_add(path)' to load an active repo scope."
            });
        }

        let codebase = self.state.codebase_path.read().clone();
        contextro_tools::search::handle_search_with_codebase(
            args,
            &self.state.bm25,
            &self.state.graph,
            &self.state.query_cache,
            &self.state.vector_index,
            codebase.as_deref(),
        )
    }

    fn handle_index(&self, args: &Value) -> Value {
        self.handle_index_internal(args, false)
    }

    fn handle_index_internal(&self, args: &Value, prewarm_commit_search: bool) -> Value {
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
                self.state.bm25.clear();
                self.state.bm25.index_chunks(&chunks);
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

    fn handle_repo_remove(&self, args: &Value) -> Value {
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

    fn remember_repo_scope(&self, previous_path: String, next_path: String) {
        if previous_path == next_path {
            return;
        }

        let mut history = self.state.repo_scope_history.write();
        history.retain(|path| path != &next_path);
        if history.last() != Some(&previous_path) {
            history.push(previous_path);
        }
    }

    fn prune_repo_scope_history(&self, removed_path: &str) {
        self.state.prune_repo_snapshot(removed_path);
        self.state
            .repo_scope_history
            .write()
            .retain(|path| path != removed_path);
    }

    fn take_previous_repo_scope_candidate(&self) -> Option<String> {
        let mut history = self.state.repo_scope_history.write();
        while let Some(candidate) = history.pop() {
            if std::path::Path::new(&candidate).is_dir() {
                return Some(candidate);
            }
        }
        None
    }

    fn repo_snapshot_matches_disk(path: &str) -> bool {
        let settings = get_settings().read().clone();
        let files = contextro_indexing::discover_files(std::path::Path::new(path), &settings);
        let current_hashes = contextro_indexing::fingerprint_files(&files);
        let storage_dir = contextro_config::project_storage_dir(path);
        let stored_hashes = contextro_indexing::load_hashes(&storage_dir);
        if stored_hashes.is_empty() {
            return false;
        }

        let (added, modified, deleted) =
            contextro_indexing::diff_file_states(&current_hashes, &stored_hashes);
        added.is_empty() && modified.is_empty() && deleted.is_empty()
    }

    fn restore_repo_snapshot(
        &self,
        path: &str,
        snapshot: &RepoScopeSnapshot,
    ) -> (Value, RestoreSnapshotMetrics) {
        let restore_start = Instant::now();
        let graph_start = Instant::now();
        self.state.graph.clear();
        if snapshot.graph.is_empty() {
            self.state.build_graph(&snapshot.symbols);
            self.state.graph.compute_pagerank();
        } else {
            self.state.graph.restore_snapshot(&snapshot.graph);
        }
        let graph_ms = graph_start.elapsed().as_secs_f64() * 1000.0;

        let bm25_start = Instant::now();
        self.state.bm25.clear();
        self.state.bm25.index_chunks(&snapshot.chunks);
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
        self.state.query_cache.invalidate();
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

    fn try_restore_cached_repo_scope(&self, path: &str) -> Option<Value> {
        let snapshot = self.load_valid_repo_snapshot(path)?;
        Some(self.restore_repo_snapshot(path, &snapshot).0)
    }

    fn load_valid_repo_snapshot(&self, path: &str) -> Option<RepoScopeSnapshot> {
        if let Some(snapshot) = self.state.repo_snapshot(path) {
            if !Self::repo_snapshot_matches_disk(path) {
                self.state.prune_repo_snapshot(path);
                return None;
            }
            return Some(snapshot);
        }

        let snapshot = self.state.load_persisted_repo_snapshot(path)?;
        if !Self::repo_snapshot_matches_disk(path) {
            self.state.prune_repo_snapshot(path);
            return None;
        }
        self.state
            .remember_repo_snapshot(path.to_string(), snapshot.clone());
        Some(snapshot)
    }

    fn load_repo_snapshot_if_hashes_match(&self, path: &str) -> Option<RepoScopeSnapshot> {
        if let Some(snapshot) = self.state.repo_snapshot(path) {
            return Some(snapshot);
        }

        let snapshot = self.state.load_persisted_repo_snapshot(path)?;
        self.state
            .remember_repo_snapshot(path.to_string(), snapshot.clone());
        Some(snapshot)
    }

    fn clear_active_scope(&self) {
        self.state.graph.clear();
        self.state.bm25.clear();
        self.state.vector_index.clear();
        self.state.query_cache.invalidate();
        self.state
            .chunk_count
            .store(0, std::sync::atomic::Ordering::Relaxed);
        *self.state.indexed.write() = false;
        *self.state.codebase_path.write() = None;
        self.state.knowledge.set_active_scope(None);
        self.state.persist_repo_scope_state();
    }

    fn restore_persisted_active_scope(&self) {
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

    fn handle_find_symbol(&self, args: &Value) -> Value {
        if !*self.state.indexed.read() {
            return json!({"error": "No codebase indexed. Run 'index' first."});
        }
        let name = args
            .get("symbol_name")
            .or_else(|| args.get("name"))
            .or_else(|| args.get("symbol"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let exact = args.get("exact").and_then(|v| v.as_bool()).unwrap_or(true);
        if name.is_empty() {
            return json!({"error": "Missing required parameter: symbol_name"});
        }

        let cb = self.state.codebase_path.read().clone();
        let matches = if exact {
            self.state.graph.find_nodes_by_name(name, true)
        } else {
            resolve_refactor_targets(name, &self.state.graph)
        };
        if matches.is_empty() {
            let mut result = json!({"error": format!("Symbol '{}' not found.", name)});
            if exact {
                result["hint"] = json!("Try exact=false for fuzzy/prefix matching if you are not sure about the full symbol name.");
                let fuzzy = self.state.graph.find_nodes_by_name(name, false);
                if !fuzzy.is_empty() {
                    result["did_you_mean"] = json!(fuzzy
                        .iter()
                        .take(3)
                        .map(|node| format!(
                            "{} ({}:{})",
                            node.name,
                            strip_codebase(&node.location.file_path, cb.as_deref()),
                            node.location.start_line
                        ))
                        .collect::<Vec<_>>());
                }
            }
            return result;
        }

        let symbols: Vec<Value> = matches.iter().take(20).map(|node| {
            let fp = cb.as_ref().map(|b| std::path::Path::new(&node.location.file_path).strip_prefix(b).map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| node.location.file_path.clone())).unwrap_or_else(|| node.location.file_path.clone());
            json!({"name": node.name, "type": node.node_type.to_string(), "file": fp, "line": node.location.start_line})
        }).collect();

        json!({"total": symbols.len(), "symbols": symbols})
    }

    /// #6: Composite tool — find_symbol + callers + impact + explain in one call.
    fn handle_refactor_check(&self, args: &Value) -> Value {
        let name = args
            .get("symbol_name")
            .or_else(|| args.get("name"))
            .or_else(|| args.get("symbol"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if name.is_empty() {
            return json!({"error": "Missing required parameter: symbol_name"});
        }
        let cb = self.state.codebase_path.read().clone();
        let codebase = cb.as_deref();
        let max_depth = args.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(3) as usize;

        let graph = &self.state.graph;
        let matches = resolve_refactor_targets(name, graph);
        if matches.is_empty() {
            return json!({"error": format!("Symbol '{}' not found.", name)});
        }

        let node = &matches[0];
        let fp = strip_codebase(&node.location.file_path, codebase);
        let (in_d, out_d) = graph.get_node_degree(&node.id);

        // Callers
        let callers: Vec<Value> = graph
            .get_callers(&node.id)
            .iter()
            .take(10)
            .map(|c| {
                json!({
                    "name": c.name,
                    "file": strip_codebase(&c.location.file_path, codebase),
                    "line": c.location.start_line,
                })
            })
            .collect();

        // Callees
        let callees: Vec<Value> = graph
            .get_callees(&node.id)
            .iter()
            .take(10)
            .map(|c| {
                json!({
                    "name": c.name,
                    "file": strip_codebase(&c.location.file_path, codebase),
                    "line": c.location.start_line,
                })
            })
            .collect();

        // Transitive impact (BFS)
        let mut impacted: Vec<Value> = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        visited.insert(node.id.clone());
        for caller in graph.get_callers(&node.id) {
            if visited.insert(caller.id.clone()) {
                queue.push_back((caller, 1usize));
            }
        }
        while let Some((n, depth)) = queue.pop_front() {
            if depth > max_depth {
                break;
            }
            impacted.push(json!({
                "name": n.name,
                "file": strip_codebase(&n.location.file_path, codebase),
                "line": n.location.start_line,
                "depth": depth,
            }));
            if impacted.len() >= 20 {
                break;
            }
            for caller in graph.get_callers(&n.id) {
                if visited.insert(caller.id.clone()) {
                    queue.push_back((caller, depth + 1));
                }
            }
        }

        let risk = if in_d > 10 {
            "high"
        } else if in_d > 3 {
            "medium"
        } else {
            "low"
        };

        json!({
            "symbol": name,
            "file": fp,
            "line": node.location.start_line,
            "type": node.node_type.to_string(),
            "docstring": node.docstring,
            "callers": callers,
            "callees": callees,
            "callers_count": in_d,
            "callees_count": out_d,
            "impacted": impacted,
            "impacted_count": impacted.len(),
            "risk": risk,
            "suggestion": if in_d > 10 {
                format!("{} callers — consider adding a deprecation alias or adapter.", in_d)
            } else if in_d > 0 {
                format!("{} callers — update all call sites after refactoring.", in_d)
            } else {
                "No callers — safe to change signature freely.".to_string()
            },
        })
    }

    /// #7: Composite tool — verify refactor completeness against the code graph.
    fn handle_completion_check(&self, args: &Value) -> Value {
        if !*self.state.indexed.read() || self.state.codebase_path.read().is_none() {
            return json!({
                "error": "No codebase loaded. Run 'index(path)' or 'repo_add(path)' to load an active repo scope."
            });
        }

        let codebase = self.state.codebase_path.read().clone();
        contextro_tools::completion::handle_completion_check(
            args,
            &self.state.graph,
            codebase.as_deref(),
        )
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn tool_definitions() -> Vec<Tool> {
        tool_registry::all_tool_definitions()
    }

    pub(crate) fn listed_tool_definitions() -> Vec<Tool> {
        tool_registry::configured_tool_definitions()
    }
}

/// #1, #4, #5, #9: Format response — strip nulls, auto-truncate large responses, apply token budget.
fn format_response(value: &Value, max_tokens: usize) -> String {
    let cleaned = strip_empty_nested(value, true);
    let output = cleaned.to_string();

    // #4: Smart max_tokens default — auto-truncate at 8000 chars (~2000 tokens) if no explicit budget
    let effective_budget = if max_tokens > 0 {
        max_tokens * 4
    } else if output.len() > 8000 {
        8000 // ~2000 tokens default cap
    } else {
        0 // no truncation needed
    };

    if effective_budget > 0 && output.len() > effective_budget {
        let budget_tokens = effective_budget / 4;
        return truncate_response_json(cleaned, effective_budget, budget_tokens).to_string();
    }
    output
}

fn truncate_response_json(value: Value, budget_chars: usize, budget_tokens: usize) -> Value {
    let mut candidate = value;
    for _ in 0..12 {
        let with_meta = add_truncation_metadata(candidate.clone(), budget_tokens);
        if with_meta.to_string().len() <= budget_chars {
            return with_meta;
        }
        candidate = shrink_json_value(candidate);
    }

    json!({
        "truncated": true,
        "hint": truncation_hint(budget_tokens),
        "summary": "Response exceeded the token budget. Narrow your query or request a larger max_tokens budget.",
    })
}

fn add_truncation_metadata(value: Value, budget_tokens: usize) -> Value {
    match value {
        Value::Object(mut map) => {
            map.insert("truncated".into(), Value::Bool(true));
            map.insert("hint".into(), Value::String(truncation_hint(budget_tokens)));
            Value::Object(map)
        }
        other => json!({
            "truncated": true,
            "hint": truncation_hint(budget_tokens),
            "value": other,
        }),
    }
}

fn truncation_hint(budget_tokens: usize) -> String {
    format!(
        "Response truncated to ~{} tokens. Use max_tokens for a different budget, or narrow your query.",
        budget_tokens
    )
}

fn shrink_json_value(value: Value) -> Value {
    match value {
        Value::String(text) => Value::String(shrink_text(text)),
        Value::Array(items) => {
            let target_len = if items.len() > 1 {
                items.len().div_ceil(2)
            } else {
                1
            };
            Value::Array(
                items
                    .into_iter()
                    .take(target_len)
                    .map(shrink_json_value)
                    .collect(),
            )
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, shrink_json_value(value)))
                .collect(),
        ),
        other => other,
    }
}

fn shrink_text(mut text: String) -> String {
    let len = text.chars().count();
    if len <= 80 {
        return text;
    }

    let target = (len * 2 / 3).max(80);
    text = text.chars().take(target.saturating_sub(1)).collect();
    text.push('…');
    text
}

fn take_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn strip_empty_nested(value: &Value, is_top_level: bool) -> Value {
    match value {
        Value::Object(map) => {
            let cleaned: serde_json::Map<String, Value> = map
                .iter()
                .filter(|(_, v)| is_top_level || !is_empty_value(v))
                .map(|(k, v)| (k.clone(), strip_empty_nested(v, false)))
                .collect();
            Value::Object(cleaned)
        }
        Value::Array(arr) => {
            Value::Array(arr.iter().map(|v| strip_empty_nested(v, false)).collect())
        }
        _ => value.clone(),
    }
}

fn is_empty_value(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(m) => m.is_empty(),
        _ => false,
    }
}

/// Strip absolute codebase prefixes from display-oriented file paths in responses,
/// while preserving identity-bearing root fields like `codebase_path` and repo `path`.
fn strip_response_paths(value: Value, base: &str) -> Value {
    let prefix = format!("{base}/");
    strip_absolute_paths_impl(value, base, &prefix, true, None)
}

fn response_needs_path_stripping(value: &Value, base: &str) -> bool {
    let prefix = format!("{base}/");
    response_contains_path_prefix(value, base, &prefix, true, None)
}

fn response_contains_path_prefix(
    value: &Value,
    base: &str,
    prefix: &str,
    preserve_identity_paths: bool,
    parent_key: Option<&str>,
) -> bool {
    match value {
        Value::String(s) => {
            if preserve_identity_paths && matches!(parent_key, Some("codebase_path" | "path")) {
                return false;
            }
            s == base || s.starts_with(prefix)
        }
        Value::Object(map) => map.iter().any(|(key, value)| {
            response_contains_path_prefix(
                value,
                base,
                prefix,
                preserve_identity_paths,
                Some(key.as_str()),
            )
        }),
        Value::Array(items) => items.iter().any(|value| {
            response_contains_path_prefix(value, base, prefix, preserve_identity_paths, parent_key)
        }),
        _ => false,
    }
}

/// Recursively replace any string value that starts with `base/` with the relative path.
fn strip_absolute_paths(value: Value, base: &str) -> Value {
    let prefix = format!("{base}/");
    strip_absolute_paths_impl(value, base, &prefix, false, None)
}

fn strip_absolute_paths_impl(
    value: Value,
    base: &str,
    prefix: &str,
    preserve_identity_paths: bool,
    parent_key: Option<&str>,
) -> Value {
    match value {
        Value::String(s) => {
            if preserve_identity_paths && matches!(parent_key, Some("codebase_path" | "path")) {
                return Value::String(s);
            }
            if let Some(stripped) = s.strip_prefix(prefix) {
                Value::String(stripped.to_string())
            } else if s == base {
                Value::String(".".to_string())
            } else {
                Value::String(s)
            }
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| {
                    let stripped = strip_absolute_paths_impl(
                        v,
                        base,
                        prefix,
                        preserve_identity_paths,
                        Some(k.as_str()),
                    );
                    (k, stripped)
                })
                .collect(),
        ),
        Value::Array(arr) => Value::Array(
            arr.into_iter()
                .map(|v| {
                    strip_absolute_paths_impl(v, base, prefix, preserve_identity_paths, parent_key)
                })
                .collect(),
        ),
        other => other,
    }
}

fn strip_codebase(path: &str, codebase: Option<&str>) -> String {
    codebase
        .and_then(|b| std::path::Path::new(path).strip_prefix(b).ok())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn summarize_tool_call(name: &str, args: &Value, codebase: Option<&str>) -> String {
    if name == "compact" {
        let chars = args
            .get("content")
            .and_then(|value| value.as_str())
            .map(|content| content.len())
            .unwrap_or(0);
        return format!("compact(chars={chars})");
    }

    let sanitized = sanitize_tool_args(args, codebase);
    let Some(map) = sanitized.as_object() else {
        return format!("{name}()");
    };
    if map.is_empty() {
        return format!("{name}()");
    }

    let mut parts: Vec<String> = map
        .iter()
        .take(3)
        .map(|(key, value)| format!("{key}={}", summarize_value(value)))
        .collect();
    if map.len() > 3 {
        parts.push("…".into());
    }
    format!("{name}({})", parts.join(", "))
}

fn sanitize_tool_args(args: &Value, codebase: Option<&str>) -> Value {
    let stripped = if let Some(base) = codebase {
        strip_absolute_paths(args.clone(), base)
    } else {
        args.clone()
    };
    truncate_json_strings(stripped, 160)
}

fn truncate_json_strings(value: Value, max_len: usize) -> Value {
    match value {
        Value::String(mut text) => {
            if text.len() > max_len {
                text.truncate(max_len);
                text.push('…');
            }
            Value::String(text)
        }
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| truncate_json_strings(item, max_len))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, truncate_json_strings(value, max_len)))
                .collect(),
        ),
        other => other,
    }
}

fn summarize_value(value: &Value) -> String {
    match value {
        Value::String(text) => format!("{text:?}"),
        Value::Number(number) => number.to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Null => "null".into(),
        Value::Array(items) => format!("[{} item(s)]", items.len()),
        Value::Object(map) => format!("{{{} key(s)}}", map.len()),
    }
}

fn edit_distance(a: &str, b: &str) -> usize {
    let (m, n) = (a.len(), b.len());
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0; n + 1];
    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(curr[j] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

impl Default for ContextroServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Scan `root` for project documentation files and add them to the knowledge store.
/// Returns the number of documents indexed. Does nothing if the KB already has content.
fn auto_populate_knowledge(root: &str, knowledge: &contextro_tools::KnowledgeStore) -> usize {
    let candidates = [
        "README.md",
        "README.txt",
        "README",
        "CLAUDE.md",
        "AGENTS.md",
        "docs/README.md",
        "docs/index.md",
        "CONTRIBUTING.md",
    ];
    if candidates.iter().any(|name| knowledge.contains(name)) {
        return 0; // KB already has seeded docs for this scope; don't overwrite
    }
    let mut count = 0;
    for name in &candidates {
        let p = std::path::Path::new(root).join(name);
        if let Ok(content) = std::fs::read_to_string(&p) {
            if !content.trim().is_empty() {
                let chunks = knowledge.add(name, &content, Some(&p));
                if chunks > 0 {
                    count += 1;
                }
            }
        }
    }
    count
}

fn should_build_vector_index(chunk_count: usize) -> bool {
    chunk_count > 1
}

fn response_cache_key(name: &str, args: &Value, codebase: Option<&str>) -> Option<String> {
    if !is_response_cacheable_tool(name) {
        return None;
    }

    let args = strip_render_only_args(args);
    Some(
        json!({
            "tool": name,
            "args": args,
            "codebase": codebase.unwrap_or(""),
        })
        .to_string(),
    )
}

fn is_response_cacheable_tool(name: &str) -> bool {
    matches!(
        name,
        "overview"
            | "architecture"
            | "analyze"
            | "focus"
            | "dead_code"
            | "circular_dependencies"
            | "test_coverage_map"
            | "audit"
    )
}

fn strip_render_only_args(args: &Value) -> Value {
    match args {
        Value::Object(map) => {
            let mut cleaned = map.clone();
            cleaned.remove("max_tokens");
            Value::Object(cleaned)
        }
        other => other.clone(),
    }
}

fn cached_tool_result(result: Value, args: &Value) -> CallToolResult {
    let max_tokens = args.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    CallToolResult::success(vec![Content::text(format_response(&result, max_tokens))])
}

fn maybe_prewarm_commit_search_cache(path: &str) {
    if !get_settings().read().search_prewarm_enabled {
        return;
    }

    contextro_tools::git_tools::prewarm_commit_search_cache(Some(path));
}

impl ServerHandler for ContextroServer {
    fn get_info(&self) -> InitializeResult {
        InitializeResult {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "contextro".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            instructions: Some("Contextro: code intelligence MCP server. 37 tools for search, graph analysis, memory, and git.".into()),
        }
    }

    fn list_tools(
        &self,
        _request: PaginatedRequestParam,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        std::future::ready(Ok(ListToolsResult {
            tools: Self::listed_tool_definitions(),
            next_cursor: None,
        }))
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        let args = request.arguments.map(Value::Object).unwrap_or(Value::Null);
        let result = self.dispatch(&request.name, args);
        std::future::ready(Ok(result))
    }
}

fn normalize_repo_dir(path: &str) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| std::path::PathBuf::from(path))
        .to_string_lossy()
        .to_string()
}

fn resolve_refactor_targets(
    name: &str,
    graph: &contextro_engines::graph::CodeGraph,
) -> Vec<contextro_core::graph::UniversalNode> {
    let exact = graph.find_nodes_by_name(name, true);
    if !exact.is_empty() {
        return rank_nodes_by_degree(exact, graph);
    }
    rank_nodes_by_degree(graph.find_nodes_by_name(name, false), graph)
}

fn rank_nodes_by_degree(
    mut nodes: Vec<contextro_core::graph::UniversalNode>,
    graph: &contextro_engines::graph::CodeGraph,
) -> Vec<contextro_core::graph::UniversalNode> {
    nodes.sort_by_key(|node| {
        let (in_degree, out_degree) = graph.get_node_degree(&node.id);
        std::cmp::Reverse(in_degree + out_degree)
    });
    nodes
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::{
        format_response, normalize_repo_dir, resolve_refactor_targets, strip_response_paths,
        take_chars, ContextroServer,
    };
    use contextro_config::Settings;
    use contextro_core::graph::{
        RelationshipType, UniversalLocation, UniversalNode, UniversalRelationship,
    };
    use contextro_core::NodeType;
    use contextro_engines::graph::CodeGraph;
    use serde_json::{json, Value};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_can_skip_reindex_only_for_same_loaded_repo() {
        assert!(ContextroServer::can_skip_reindex(
            "/tmp/repo-a",
            Some("/tmp/repo-a"),
            true,
            true,
            0,
        ));

        assert!(!ContextroServer::can_skip_reindex(
            "/tmp/repo-a",
            Some("/tmp/repo-b"),
            true,
            true,
            0,
        ));

        assert!(!ContextroServer::can_skip_reindex(
            "/tmp/repo-a",
            None,
            true,
            true,
            0,
        ));
    }

    #[test]
    fn test_format_response_truncation_stays_valid_json() {
        let value = json!({
            "symbol": "BrowserSession.close",
            "callers": (0..120)
                .map(|i| format!("caller_{i} (tests/file_{i}.py:{})", i + 1))
                .collect::<Vec<_>>(),
            "total": 120,
        });

        let rendered = format_response(&value, 200);
        let parsed: Value =
            serde_json::from_str(&rendered).expect("truncated output should stay valid JSON");

        assert_eq!(parsed["symbol"], "BrowserSession.close");
        assert_eq!(parsed["total"], 120);
        assert_eq!(parsed["truncated"], true);
        assert!(parsed["callers"].as_array().unwrap().len() < 120);
        assert!(parsed["hint"]
            .as_str()
            .unwrap()
            .contains("Response truncated"));
    }

    #[test]
    fn test_resolve_refactor_targets_supports_qualified_method_names() {
        let graph = CodeGraph::new();
        graph.add_node(UniversalNode {
            id: "class-browser-session".into(),
            name: "BrowserSession".into(),
            node_type: NodeType::Class,
            location: UniversalLocation {
                file_path: "/tmp/repo/session.py".into(),
                start_line: 1,
                end_line: 20,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            ..Default::default()
        });
        graph.add_node(UniversalNode {
            id: "method-close".into(),
            name: "close".into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: "/tmp/repo/session.py".into(),
                start_line: 22,
                end_line: 30,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            parent: Some("BrowserSession".into()),
            ..Default::default()
        });
        graph.add_node(UniversalNode {
            id: "caller".into(),
            name: "shutdown".into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: "/tmp/repo/main.py".into(),
                start_line: 5,
                end_line: 12,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            ..Default::default()
        });
        graph.add_relationship(UniversalRelationship {
            id: "calls-close".into(),
            source_id: "caller".into(),
            target_id: "method-close".into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });

        let matches = resolve_refactor_targets("BrowserSession.close", &graph);
        assert!(!matches.is_empty());
        assert_eq!(matches[0].name, "close");
        assert_eq!(matches[0].parent.as_deref(), Some("BrowserSession"));
    }

    #[test]
    fn test_strip_response_paths_preserves_root_identity_fields() {
        let base = "/tmp/contextro-repo";
        let stripped = strip_response_paths(
            json!({
                "codebase_path": base,
                "path": base,
                "file": format!("{base}/src/lib.rs"),
                "repos": [{"path": base, "name": "repo-a"}],
                "nested": {"file": format!("{base}/src/main.rs")},
            }),
            base,
        );

        assert_eq!(stripped["codebase_path"], base);
        assert_eq!(stripped["path"], base);
        assert_eq!(stripped["repos"][0]["path"], base);
        assert_eq!(stripped["file"], "src/lib.rs");
        assert_eq!(stripped["nested"]["file"], "src/main.rs");
    }

    #[test]
    fn test_take_chars_handles_unicode_boundaries() {
        assert_eq!(take_chars("─alpha", 1), "─");
        assert_eq!(take_chars("hello", 4), "hell");
    }

    #[test]
    fn test_all_tool_definitions_expose_object_input_schema() {
        for tool in ContextroServer::tool_definitions() {
            let schema = tool.schema_as_json_value();
            assert_eq!(
                schema.get("type").and_then(Value::as_str),
                Some("object"),
                "tool '{}' must expose an object input schema: {}",
                tool.name,
                schema
            );
        }
    }

    #[test]
    fn test_find_symbol_missing_exact_match_suggests_fuzzy_lookup() {
        let server = ContextroServer::new();
        server
            .state
            .graph
            .add_node(contextro_core::graph::UniversalNode {
                id: "browser-session".into(),
                name: "BrowserSession".into(),
                node_type: contextro_core::NodeType::Class,
                location: contextro_core::graph::UniversalLocation {
                    file_path: "/tmp/repo/src/browser/session.py".into(),
                    start_line: 1,
                    end_line: 20,
                    start_column: 0,
                    end_column: 0,
                    language: "python".into(),
                },
                language: "python".into(),
                ..Default::default()
            });
        *server.state.indexed.write() = true;
        let result = server.handle_find_symbol(&json!({"symbol_name":"Browser","exact":true}));

        assert_eq!(result["error"], "Symbol 'Browser' not found.");
        assert!(result["hint"]
            .as_str()
            .unwrap_or("")
            .contains("exact=false"));
        assert!(result["did_you_mean"].is_array());
    }

    fn temp_repo_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("contextro-server-{unique}-{name}"))
    }

    fn write_indexable_repo(root: &Path, symbol_name: &str) {
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/lib.rs"),
            format!("pub fn {symbol_name}() {{}}\n"),
        )
        .unwrap();
    }

    fn temp_storage_dir(name: &str) -> PathBuf {
        temp_repo_dir(&format!("storage-{name}"))
    }

    fn test_settings(storage_dir: &Path) -> Settings {
        Settings {
            storage_dir: storage_dir.to_string_lossy().to_string(),
            ..Settings::default()
        }
    }

    fn test_server(storage_dir: &Path) -> ContextroServer {
        fs::create_dir_all(storage_dir).unwrap();
        ContextroServer::with_settings(test_settings(storage_dir))
    }

    #[test]
    fn test_repo_remove_restores_previous_active_scope_and_knowledge_scope() {
        let storage_dir = temp_storage_dir("repo-remove-restore");
        let server = test_server(&storage_dir);
        let repo_a = temp_repo_dir("repo-a");
        let repo_b = temp_repo_dir("repo-b");
        write_indexable_repo(&repo_a, "repo_a_symbol");
        write_indexable_repo(&repo_b, "repo_b_symbol");

        let index_a = server.handle_index(&json!({"path": repo_a.to_string_lossy().to_string()}));
        assert_eq!(index_a["status"], "done");
        server
            .state
            .knowledge
            .add("repo-a-doc", "alpha scope", None);

        server.dispatch(
            "repo_add",
            json!({"path": repo_b.to_string_lossy().to_string()}),
        );
        assert_eq!(
            server
                .state
                .codebase_path
                .read()
                .clone()
                .map(|path| normalize_repo_dir(&path)),
            Some(normalize_repo_dir(repo_b.to_string_lossy().as_ref()))
        );
        server
            .state
            .knowledge
            .add("repo-b-doc", "bravo scope", None);

        let remove_result =
            server.handle_repo_remove(&json!({"path": repo_b.to_string_lossy().to_string()}));

        assert_eq!(remove_result["removed"], true);
        assert_eq!(remove_result["active_scope_restored"], true);
        assert_eq!(
            server
                .state
                .codebase_path
                .read()
                .clone()
                .map(|path| normalize_repo_dir(&path)),
            Some(normalize_repo_dir(repo_a.to_string_lossy().as_ref()))
        );
        assert_eq!(server.state.knowledge.search("alpha", 5).len(), 1);
        assert!(server.state.knowledge.search("bravo", 5).is_empty());

        let restored_codebase = server.state.codebase_path.read().clone();
        let overview = contextro_tools::analysis::handle_overview(
            &server.state.graph,
            restored_codebase.as_deref(),
            server
                .state
                .chunk_count
                .load(std::sync::atomic::Ordering::Relaxed),
            server.state.vector_index.len(),
        );
        let architecture = contextro_tools::analysis::handle_architecture(
            &json!({}),
            &server.state.graph,
            restored_codebase.as_deref(),
        );
        let repo_a_search = contextro_tools::search::handle_search(
            &json!({"query": "repo_a_symbol"}),
            &server.state.bm25,
            &server.state.graph,
            &server.state.query_cache,
            &server.state.vector_index,
        );
        let repo_b_search = contextro_tools::search::handle_search(
            &json!({"query": "repo_b_symbol"}),
            &server.state.bm25,
            &server.state.graph,
            &server.state.query_cache,
            &server.state.vector_index,
        );
        let repo_a_results = repo_a_search["results"].as_array().expect("results array");
        let repo_b_results = repo_b_search["results"].as_array().expect("results array");

        assert_eq!(
            overview["codebase_path"],
            normalize_repo_dir(repo_a.to_string_lossy().as_ref())
        );
        assert!(overview["total_symbols"].as_u64().unwrap_or(0) >= 1);
        assert!(architecture["total_nodes"].as_u64().unwrap_or(0) >= 1);
        assert!(repo_a_search["total"].as_u64().unwrap_or(0) >= 1);
        assert!(repo_a_results
            .iter()
            .any(|result| result["name"] == "repo_a_symbol"));
        assert!(!repo_b_results
            .iter()
            .any(|result| result["name"] == "repo_b_symbol"));

        let _ = std::fs::remove_dir_all(repo_a);
        let _ = std::fs::remove_dir_all(repo_b);
        let _ = std::fs::remove_dir_all(storage_dir);
    }

    #[test]
    fn test_repo_remove_reindexes_previous_scope_when_cached_snapshot_is_stale() {
        let storage_dir = temp_storage_dir("repo-remove-stale-snapshot");
        let server = test_server(&storage_dir);
        let repo_a = temp_repo_dir("repo-stale-a");
        let repo_b = temp_repo_dir("repo-stale-b");
        write_indexable_repo(&repo_a, "repo_a_symbol");
        write_indexable_repo(&repo_b, "repo_b_symbol");

        let index_a = server.handle_index(&json!({"path": repo_a.to_string_lossy().to_string()}));
        assert_eq!(index_a["status"], "done");
        let add_b = server.dispatch(
            "repo_add",
            json!({"path": repo_b.to_string_lossy().to_string()}),
        );
        assert_ne!(add_b.is_error, Some(true));

        write_indexable_repo(&repo_a, "repo_a_updated_symbol");

        let remove_result =
            server.handle_repo_remove(&json!({"path": repo_b.to_string_lossy().to_string()}));
        assert_eq!(remove_result["removed"], true);
        assert_eq!(remove_result["active_scope_restored"], true);

        let search_updated = server.handle_search(&json!({"query": "repo_a_updated_symbol"}));
        let updated_results = search_updated["results"].as_array().expect("results array");
        assert!(updated_results
            .iter()
            .any(|result| result["name"] == "repo_a_updated_symbol"));

        let search_old = server.handle_search(&json!({"query": "repo_a_symbol"}));
        let old_results = search_old["results"].as_array().expect("results array");
        assert!(!old_results
            .iter()
            .any(|result| result["name"] == "repo_a_symbol"));

        let _ = std::fs::remove_dir_all(repo_a);
        let _ = std::fs::remove_dir_all(repo_b);
        let _ = std::fs::remove_dir_all(storage_dir);
    }

    #[test]
    fn test_repo_remove_clears_active_scope_when_no_previous_scope_exists() {
        let storage_dir = temp_storage_dir("repo-remove-clear");
        let server = test_server(&storage_dir);
        let repo = temp_repo_dir("repo-clear");
        write_indexable_repo(&repo, "repo_clear_symbol");

        server
            .state
            .repo_registry
            .add(repo.to_string_lossy().as_ref(), None);
        let index_result =
            server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
        assert_eq!(index_result["status"], "done");
        server
            .state
            .knowledge
            .add("repo-doc", "repo scoped note", None);

        let remove_result =
            server.handle_repo_remove(&json!({"path": repo.to_string_lossy().to_string()}));

        assert_eq!(remove_result["removed"], true);
        assert_eq!(remove_result["active_scope_cleared"], true);
        assert!(!(*server.state.indexed.read()));
        assert_eq!(*server.state.codebase_path.read(), None);
        assert_eq!(server.state.graph.node_count(), 0);
        assert_eq!(
            server
                .state
                .chunk_count
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
        assert!(server.state.knowledge.search("repo scoped", 5).is_empty());
        assert!(remove_result["warning"]
            .as_str()
            .unwrap_or("")
            .contains("no previous repo scope"));

        let overview = contextro_tools::analysis::handle_overview(
            &server.state.graph,
            server.state.codebase_path.read().as_deref(),
            server
                .state
                .chunk_count
                .load(std::sync::atomic::Ordering::Relaxed),
            server.state.vector_index.len(),
        );
        let architecture = contextro_tools::analysis::handle_architecture(
            &json!({}),
            &server.state.graph,
            server.state.codebase_path.read().as_deref(),
        );
        let search = contextro_tools::search::handle_search(
            &json!({"query": "repo_clear_symbol"}),
            &server.state.bm25,
            &server.state.graph,
            &server.state.query_cache,
            &server.state.vector_index,
        );

        assert_eq!(overview["codebase_path"], Value::Null);
        assert_eq!(overview["total_symbols"], 0);
        assert_eq!(architecture["total_nodes"], 0);
        assert_eq!(search["total"], 0);

        let _ = std::fs::remove_dir_all(repo);
        let _ = std::fs::remove_dir_all(storage_dir);
    }

    #[test]
    fn test_handle_index_restores_from_persisted_snapshot_when_repo_is_unchanged() {
        let initial_storage_dir = temp_storage_dir("persisted-snapshot-source");
        let restore_storage_dir = temp_storage_dir("persisted-snapshot-restore");
        let repo = temp_repo_dir("persisted-snapshot-repo");
        write_indexable_repo(&repo, "persisted_snapshot_symbol");

        let server = test_server(&initial_storage_dir);
        let initial_index =
            server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
        assert_eq!(initial_index["status"], "done");
        let normalized_repo = normalize_repo_dir(repo.to_string_lossy().as_ref());
        let project_storage = contextro_config::project_storage_dir(&normalized_repo);
        assert!(!contextro_indexing::load_hashes(&project_storage).is_empty());
        assert!(server
            .state
            .load_persisted_repo_snapshot(&normalized_repo)
            .is_some());

        let restored_server = test_server(&restore_storage_dir);
        let restored =
            restored_server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
        assert_eq!(restored["status"], "done");
        assert_eq!(restored["index_mode"], "restored");
        assert_eq!(restored["restored_from_cache"], true);
        assert_eq!(
            restored["message"],
            "Restored from persisted repo snapshot."
        );
        assert!(restored["restore_ms"].as_f64().unwrap_or(0.0) >= 0.0);
        assert!(restored["request_ms"].as_f64().unwrap_or(0.0) >= 0.0);

        let search = restored_server.handle_search(&json!({"query": "persisted_snapshot_symbol"}));
        let results = search["results"].as_array().expect("results array");
        assert!(results
            .iter()
            .any(|result| result["name"] == "persisted_snapshot_symbol"));

        let _ = std::fs::remove_dir_all(repo.clone());
        let _ = std::fs::remove_dir_all(initial_storage_dir);
        let _ = std::fs::remove_dir_all(restore_storage_dir);
        let _ = std::fs::remove_dir_all(project_storage);
    }

    #[test]
    fn test_handle_index_rebuilds_when_persisted_snapshot_is_stale() {
        let initial_storage_dir = temp_storage_dir("persisted-snapshot-stale-source");
        let restore_storage_dir = temp_storage_dir("persisted-snapshot-stale-restore");
        let repo = temp_repo_dir("persisted-snapshot-stale-repo");
        write_indexable_repo(&repo, "stale_snapshot_symbol");

        let server = test_server(&initial_storage_dir);
        let initial_index =
            server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
        assert_eq!(initial_index["status"], "done");

        write_indexable_repo(&repo, "fresh_snapshot_symbol");

        let restored_server = test_server(&restore_storage_dir);
        let restored =
            restored_server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
        assert_eq!(restored["status"], "done");
        assert_eq!(restored["index_mode"], "fresh");
        assert_ne!(restored.get("restored_from_cache"), Some(&json!(true)));
        assert!(restored["graph_ms"].as_f64().unwrap_or(0.0) >= 0.0);
        assert!(restored["request_ms"].as_f64().unwrap_or(0.0) >= 0.0);

        let fresh_search =
            restored_server.handle_search(&json!({"query": "fresh_snapshot_symbol"}));
        let fresh_results = fresh_search["results"].as_array().expect("results array");
        assert!(fresh_results
            .iter()
            .any(|result| result["name"] == "fresh_snapshot_symbol"));

        let stale_search =
            restored_server.handle_search(&json!({"query": "stale_snapshot_symbol"}));
        let stale_results = stale_search["results"].as_array().expect("results array");
        assert!(!stale_results
            .iter()
            .any(|result| result["name"] == "stale_snapshot_symbol"));

        let _ = std::fs::remove_dir_all(repo.clone());
        let _ = std::fs::remove_dir_all(initial_storage_dir);
        let _ = std::fs::remove_dir_all(restore_storage_dir);
        let _ = std::fs::remove_dir_all(contextro_config::project_storage_dir(
            &normalize_repo_dir(repo.to_string_lossy().as_ref()),
        ));
    }

    #[test]
    fn test_restore_repo_snapshot_falls_back_when_graph_snapshot_is_missing() {
        let storage_dir = temp_storage_dir("persisted-snapshot-legacy-graph");
        let repo = temp_repo_dir("persisted-snapshot-legacy-repo");
        write_indexable_repo(&repo, "legacy_snapshot_symbol");

        let server = test_server(&storage_dir);
        let indexed = server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
        assert_eq!(indexed["status"], "done");

        let normalized_repo = normalize_repo_dir(repo.to_string_lossy().as_ref());
        let mut snapshot = server
            .state
            .load_persisted_repo_snapshot(&normalized_repo)
            .expect("snapshot should exist");
        snapshot.graph = contextro_engines::graph::GraphSnapshot::default();

        server.clear_active_scope();
        let restored = server.restore_repo_snapshot(&normalized_repo, &snapshot).0;
        assert_eq!(restored["status"], "done");
        assert_eq!(restored["restored_from_cache"], true);

        let search = server.handle_search(&json!({"query": "legacy_snapshot_symbol"}));
        let results = search["results"].as_array().expect("results array");
        assert!(results
            .iter()
            .any(|result| result["name"] == "legacy_snapshot_symbol"));

        let _ = std::fs::remove_dir_all(repo.clone());
        let _ = std::fs::remove_dir_all(storage_dir);
        let _ = std::fs::remove_dir_all(contextro_config::project_storage_dir(&normalized_repo));
    }

    #[test]
    fn test_restart_restores_active_scope_and_search_after_repo_add() {
        let storage_dir = temp_storage_dir("restart-repo-add");
        let repo = temp_repo_dir("restart-repo-a");
        write_indexable_repo(&repo, "restart_repo_symbol");

        let server = test_server(&storage_dir);
        let add_result = server.dispatch(
            "repo_add",
            json!({"path": repo.to_string_lossy().to_string()}),
        );
        assert_ne!(add_result.is_error, Some(true));

        let restarted = test_server(&storage_dir);
        assert!(*restarted.state.indexed.read());
        assert_eq!(
            restarted
                .state
                .codebase_path
                .read()
                .clone()
                .map(|path| normalize_repo_dir(&path)),
            Some(normalize_repo_dir(repo.to_string_lossy().as_ref()))
        );

        let search = restarted.handle_search(&json!({"query": "restart_repo_symbol"}));
        let results = search["results"].as_array().expect("results array");
        assert!(results
            .iter()
            .any(|result| result["name"] == "restart_repo_symbol"));

        let _ = std::fs::remove_dir_all(repo);
        let _ = std::fs::remove_dir_all(storage_dir);
    }

    #[test]
    fn test_repo_add_dispatch_replaces_stale_git_hint_after_auto_index() {
        let storage_dir = temp_storage_dir("repo-add-hint");
        let server = test_server(&storage_dir);
        let repo = temp_repo_dir("repo-add-hint-repo");
        write_indexable_repo(&repo, "repo_add_hint_symbol");
        let git_init = std::process::Command::new("git")
            .arg("init")
            .arg(&repo)
            .output()
            .expect("initialize git repo");
        assert!(git_init.status.success());

        let result = server.dispatch(
            "repo_add",
            json!({"path": repo.to_string_lossy().to_string()}),
        );
        assert_ne!(result.is_error, Some(true));

        let content = serde_json::to_value(&result.content[0]).expect("serialize tool content");
        let text = content["text"].as_str().expect("tool text payload");
        let payload: Value = serde_json::from_str(text).expect("repo_add JSON payload");

        assert_eq!(payload["registered"], true);
        assert_eq!(payload["indexed"], true);
        assert_eq!(
            payload["hint"],
            "Repository registered, indexed, and set as the active repo scope."
        );
        assert!(
            !text.contains("Run index(path) to build the graph and enable search for this repo.")
        );

        let _ = std::fs::remove_dir_all(repo);
        let _ = std::fs::remove_dir_all(storage_dir);
    }

    #[test]
    fn test_index_skips_vector_index_for_single_chunk_repo() {
        let storage_dir = temp_storage_dir("single-chunk-vector-skip");
        let server = test_server(&storage_dir);
        let repo = temp_repo_dir("single-chunk-repo");
        write_indexable_repo(&repo, "single_chunk_symbol");

        let result = server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));

        assert_eq!(result["status"], "done");
        assert_eq!(result["index_mode"], "fresh");
        assert_eq!(result["total_chunks"], 1);
        assert_eq!(result["vector_chunks"], 0);

        let search = server.handle_search(&json!({"query": "single_chunk_symbol"}));
        let results = search["results"].as_array().expect("results array");
        assert!(results
            .iter()
            .any(|entry| entry["name"] == "single_chunk_symbol"));

        let _ = std::fs::remove_dir_all(repo);
        let _ = std::fs::remove_dir_all(storage_dir);
    }

    #[test]
    fn test_handle_index_skips_reindex_for_unchanged_loaded_repo() {
        let storage_dir = temp_storage_dir("index-skip-unchanged");
        let server = test_server(&storage_dir);
        let repo = temp_repo_dir("index-skip-unchanged-repo");
        write_indexable_repo(&repo, "skip_unchanged_symbol");

        let indexed = server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
        assert_eq!(indexed["status"], "done");
        assert_eq!(indexed["index_mode"], "fresh");

        let skipped = server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
        assert_eq!(skipped["status"], "done");
        assert_eq!(skipped["index_mode"], "skipped");
        assert_eq!(skipped["message"], "No files changed since last index.");
        assert!(skipped["request_ms"].as_f64().unwrap_or(0.0) >= 0.0);

        let _ = std::fs::remove_dir_all(repo);
        let _ = std::fs::remove_dir_all(storage_dir);
    }

    #[test]
    fn test_restart_repo_remove_restores_previous_scope() {
        let storage_dir = temp_storage_dir("restart-repo-restore");
        let repo_a = temp_repo_dir("restart-restore-a");
        let repo_b = temp_repo_dir("restart-restore-b");
        write_indexable_repo(&repo_a, "restore_repo_a_symbol");
        write_indexable_repo(&repo_b, "restore_repo_b_symbol");

        let server = test_server(&storage_dir);
        let index_a = server.handle_index(&json!({"path": repo_a.to_string_lossy().to_string()}));
        assert_eq!(index_a["status"], "done");
        let add_b = server.dispatch(
            "repo_add",
            json!({"path": repo_b.to_string_lossy().to_string()}),
        );
        assert_ne!(add_b.is_error, Some(true));

        let restarted = test_server(&storage_dir);
        let remove_result =
            restarted.handle_repo_remove(&json!({"path": repo_b.to_string_lossy().to_string()}));
        assert_eq!(remove_result["removed"], true);
        assert_eq!(remove_result["active_scope_restored"], true);
        assert_eq!(
            restarted
                .state
                .codebase_path
                .read()
                .clone()
                .map(|path| normalize_repo_dir(&path)),
            Some(normalize_repo_dir(repo_a.to_string_lossy().as_ref()))
        );

        let search = restarted.handle_search(&json!({"query": "restore_repo_a_symbol"}));
        let results = search["results"].as_array().expect("results array");
        assert!(results
            .iter()
            .any(|result| result["name"] == "restore_repo_a_symbol"));

        let _ = std::fs::remove_dir_all(repo_a);
        let _ = std::fs::remove_dir_all(repo_b);
        let _ = std::fs::remove_dir_all(storage_dir);
    }

    #[test]
    fn test_restart_repo_remove_only_active_repo_clears_persisted_scope() {
        let storage_dir = temp_storage_dir("restart-repo-clear");
        let repo = temp_repo_dir("restart-clear-a");
        write_indexable_repo(&repo, "restart_clear_symbol");

        let server = test_server(&storage_dir);
        let add_result = server.dispatch(
            "repo_add",
            json!({"path": repo.to_string_lossy().to_string()}),
        );
        assert_ne!(add_result.is_error, Some(true));

        let restarted = test_server(&storage_dir);
        let remove_result =
            restarted.handle_repo_remove(&json!({"path": repo.to_string_lossy().to_string()}));
        assert_eq!(remove_result["removed"], true);
        assert_eq!(remove_result["active_scope_cleared"], true);
        assert!(!(*restarted.state.indexed.read()));
        assert_eq!(*restarted.state.codebase_path.read(), None);
        assert!(!storage_dir.join("repo-scope.json").exists());

        let restarted_again = test_server(&storage_dir);
        assert!(!(*restarted_again.state.indexed.read()));
        assert_eq!(*restarted_again.state.codebase_path.read(), None);

        let _ = std::fs::remove_dir_all(repo);
        let _ = std::fs::remove_dir_all(storage_dir);
    }

    #[test]
    fn test_search_returns_clear_error_when_no_codebase_loaded() {
        let storage_dir = temp_storage_dir("search-empty-state");
        let server = test_server(&storage_dir);

        let result = server.handle_search(&json!({"query": "anything"}));

        assert_eq!(
            result["error"],
            "No codebase loaded. Run 'index(path)' or 'repo_add(path)' to load an active repo scope."
        );

        let _ = std::fs::remove_dir_all(storage_dir);
    }

    #[test]
    fn test_graph_analysis_dispatch_reuses_cached_response_across_max_tokens_variants() {
        let storage_dir = temp_storage_dir("analysis-response-cache");
        let server = test_server(&storage_dir);
        let repo = temp_repo_dir("analysis-response-cache-repo");
        write_indexable_repo(&repo, "cached_analysis_symbol");

        let indexed = server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
        assert_eq!(indexed["status"], "done");

        let cache_hit_rate_before = server.state.query_cache.hit_rate();
        let session_total_before = contextro_tools::session::handle_session_snapshot(
            &json!({"limit": 20, "type": "overview"}),
            &server.state.session_tracker,
        )["total"]
            .as_u64()
            .unwrap_or(0);
        let first = server.dispatch("overview", json!({"max_tokens": 1}));
        assert_ne!(first.is_error, Some(true));

        let second = server.dispatch("overview", json!({"max_tokens": 50}));
        assert_ne!(second.is_error, Some(true));
        assert!(
            server.state.query_cache.hit_rate() > cache_hit_rate_before,
            "expected cached overview response to increase hit rate"
        );
        let session_total_after = contextro_tools::session::handle_session_snapshot(
            &json!({"limit": 20, "type": "overview"}),
            &server.state.session_tracker,
        )["total"]
            .as_u64()
            .unwrap_or(0);
        assert_eq!(session_total_after, session_total_before + 2);

        let _ = std::fs::remove_dir_all(repo);
        let _ = std::fs::remove_dir_all(storage_dir);
    }

    #[test]
    fn test_graph_analysis_cache_invalidates_after_reindex() {
        let storage_dir = temp_storage_dir("analysis-response-cache-invalidation");
        let server = test_server(&storage_dir);
        let repo = temp_repo_dir("analysis-response-cache-invalidation-repo");
        write_indexable_repo(&repo, "cache_before_symbol");

        let indexed = server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
        assert_eq!(indexed["status"], "done");

        let first = server.dispatch("overview", json!({}));
        assert_ne!(first.is_error, Some(true));
        let hit_rate_before = server.state.query_cache.hit_rate();

        write_indexable_repo(&repo, "cache_after_symbol");
        let reindexed = server.handle_index(&json!({"path": repo.to_string_lossy().to_string()}));
        assert_eq!(reindexed["status"], "done");

        let second = server.dispatch("overview", json!({}));
        assert_ne!(second.is_error, Some(true));
        assert_eq!(server.state.query_cache.hit_rate(), hit_rate_before);

        let _ = std::fs::remove_dir_all(repo);
        let _ = std::fs::remove_dir_all(storage_dir);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Handle --version / -V before starting the server.
    let cli_args: Vec<String> = std::env::args().collect();
    if cli_args.iter().any(|a| a == "--version" || a == "-V") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    info!(
        "Starting Contextro MCP server v{}",
        env!("CARGO_PKG_VERSION")
    );

    update_check::spawn();

    let server = ContextroServer::new();
    let transport = get_settings().read().transport.clone();

    match transport.as_str() {
        "http" => {
            let (host, port) = {
                let settings = get_settings().read();
                (settings.http_host.clone(), settings.http_port)
            };
            info!("HTTP transport on {}:{}", host, port);
            http::serve_http(server, &host, port).await?;
        }
        _ => {
            let service = server
                .serve(rmcp::transport::stdio())
                .await
                .map_err(|e| anyhow::anyhow!("{:?}", e))?;
            service.waiting().await?;
        }
    }
    Ok(())
}
