//! Contextro MCP server binary — single compiled Rust binary.

use std::sync::Arc;

use anyhow::Result;
use rmcp::model::*;
use rmcp::Error as McpError;
use rmcp::{ServerHandler, ServiceExt};
use serde_json::{json, Value};
use tracing::info;

use contextro_config::get_settings;

mod http;
mod state;
mod update_check;
use state::AppState;

/// The Contextro MCP server.
#[derive(Clone)]
pub struct ContextroServer {
    state: Arc<AppState>,
}

impl ContextroServer {
    pub fn new() -> Self {
        Self {
            state: Arc::new(AppState::new()),
        }
    }

    fn dispatch(&self, name: &str, args: Value) -> CallToolResult {
        let s = &self.state;
        let codebase = s.codebase_path.read().clone();
        let cb = codebase.as_deref();

        let result = match name {
            "status" => self.handle_status(),
            "health" => self.handle_health(),
            "index" => self.handle_index(&args),
            "search" => contextro_tools::search::handle_search(
                &args,
                &s.bm25,
                &s.graph,
                &s.query_cache,
                &s.vector_index,
            ),
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
            "architecture" => contextro_tools::analysis::handle_architecture(&s.graph, cb),
            "analyze" => contextro_tools::analysis::handle_analyze(&args, &s.graph, cb),
            "focus" => contextro_tools::analysis::handle_focus(&args, &s.graph, cb),
            "dead_code" => contextro_tools::analysis::handle_dead_code(&s.graph, cb),
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
                contextro_tools::session::handle_session_snapshot(&s.session_tracker)
            }
            "restore" => contextro_tools::session::handle_restore(
                cb,
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
                    let index_result = self.handle_index(&args);
                    let mut combined = reg_result;
                    if index_result.get("status") == Some(&json!("done")) {
                        combined["indexed"] = json!(true);
                        combined["graph_nodes"] = index_result["graph_nodes"].clone();
                        combined["graph_relationships"] =
                            index_result["graph_relationships"].clone();
                        combined["total_symbols"] = index_result["total_symbols"].clone();
                    }
                    combined
                }
            }
            "repo_remove" => {
                contextro_tools::git_tools::handle_repo_remove(&args, &s.repo_registry)
            }
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
            _ => {
                json!({"error": format!("Unknown tool: '{}'. Use introspect() to find the right tool.", name)})
            }
        };

        s.session_tracker.track(
            name,
            &summarize_tool_call(name, &args, cb),
            Some(sanitize_tool_args(&args, cb)),
        );

        // ── Response optimization (#1, #5, #7, #9) ──────────────────────────
        let max_tokens = args.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

        // #5: Strip absolute codebase prefix from all file paths in response
        let result = if let Some(base) = cb {
            strip_absolute_paths(result, base)
        } else {
            result
        };

        if result.get("error").is_some() {
            // #8: Actionable errors — add fuzzy suggestions for symbol-not-found
            let err_text = result["error"].as_str().unwrap_or("");
            let enhanced = if err_text.contains("not found") {
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
                        json!({"error": err_text, "did_you_mean": sugg, "hint": format!("Try: find_symbol(symbol_name=\"{}\", exact=false)", &sym[..sym.len().min(4)])})
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
        })
    }

    fn handle_index(&self, args: &Value) -> Value {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        if path.is_empty() {
            return json!({"error": "Missing required parameter: path"});
        }
        if !std::path::Path::new(path).is_dir() {
            return json!({"error": format!("Not a directory: {}", path)});
        }

        let settings = get_settings().read().clone();
        let storage_dir = contextro_config::project_storage_dir(path);
        std::fs::create_dir_all(&storage_dir).ok();

        let pipeline = contextro_indexing::IndexingPipeline::new(settings.clone());

        // #1: Incremental re-indexing — check if files changed since last index
        let files = contextro_indexing::discover_files(std::path::Path::new(path), &settings);
        let current_hashes = contextro_indexing::hash_files(&files);
        let stored_hashes = contextro_indexing::load_hashes(&storage_dir);
        let (added, modified, deleted) =
            contextro_indexing::diff_file_states(&current_hashes, &stored_hashes);
        let changed_count = added.len() + modified.len() + deleted.len();
        let is_incremental = !stored_hashes.is_empty();

        // If nothing changed and we already have an index, skip re-parsing
        if is_incremental && changed_count == 0 && *self.state.indexed.read() {
            return json!({
                "status": "done",
                "message": "No files changed since last index.",
                "total_files": files.len(),
                "incremental": {"files_added": 0, "files_modified": 0, "files_deleted": 0, "files_unchanged": files.len()},
                "graph_nodes": self.state.graph.node_count(),
                "graph_relationships": self.state.graph.relationship_count(),
            });
        }

        match pipeline.index(std::path::Path::new(path)) {
            Ok((result, symbols)) => {
                self.state.graph.clear();
                self.state.build_graph(&symbols);
                self.state.graph.compute_pagerank();

                // Index chunks into the shared BM25 engine
                self.state.bm25.clear();
                let chunks = contextro_indexing::create_chunks(&symbols);

                // Save hashes for next incremental run
                contextro_indexing::save_hashes(&current_hashes, &storage_dir);
                self.state.bm25.index_chunks(&chunks);
                self.state
                    .chunk_count
                    .store(chunks.len(), std::sync::atomic::Ordering::Relaxed);

                // Populate vector index
                self.state.vector_index.clear();
                let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
                if let Some(vectors) = contextro_indexing::embed_batch(&texts) {
                    for (chunk, vector) in chunks.iter().zip(vectors.into_iter()) {
                        let sr = contextro_core::models::SearchResult {
                            id: chunk.id.clone(),
                            filepath: chunk.filepath.clone(),
                            symbol_name: chunk.symbol_name.clone(),
                            symbol_type: chunk.symbol_type.clone(),
                            language: chunk.language.clone(),
                            line_start: chunk.line_start,
                            line_end: chunk.line_end,
                            score: 0.0,
                            code: String::new(),
                            signature: chunk.signature.clone(),
                            match_sources: vec!["vector".into()],
                        };
                        self.state.vector_index.insert(vector, sr);
                    }
                }

                // Swap in the persistent BM25 engine
                *self.state.indexed.write() = true;
                *self.state.codebase_path.write() = Some(path.to_string());
                self.state.query_cache.invalidate();

                // Auto-populate knowledge base with project docs
                let kb_populated = auto_populate_knowledge(path, &self.state.knowledge);

                let mut resp = json!({
                    "status": "done",
                    "total_files": result.total_files,
                    "total_symbols": result.total_symbols,
                    "total_chunks": chunks.len(),
                    "graph_nodes": self.state.graph.node_count(),
                    "graph_relationships": self.state.graph.relationship_count(),
                    "vector_chunks": self.state.vector_index.len(),
                    "time_seconds": (result.time_seconds * 100.0).round() / 100.0,
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
        let matches = self.state.graph.find_nodes_by_name(name, exact);
        if matches.is_empty() {
            return json!({"error": format!("Symbol '{}' not found.", name)});
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
        let matches = graph.find_nodes_by_name(name, true);
        if matches.is_empty() {
            let fuzzy = graph.find_nodes_by_name(name, false);
            if !fuzzy.is_empty() {
                let sugg: Vec<String> = fuzzy.iter().take(3).map(|n| n.name.clone()).collect();
                return json!({"error": format!("Symbol '{}' not found.", name), "did_you_mean": sugg});
            }
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

    fn tool_definitions() -> Vec<Tool> {
        let empty: Arc<serde_json::Map<String, Value>> = Arc::new(serde_json::Map::new());
        let mk = |schema_json: &str| -> Arc<serde_json::Map<String, Value>> {
            Arc::new(serde_json::from_str(schema_json).unwrap_or_default())
        };

        // All schemas use current param names; backward-compat aliases are handled in dispatch.
        let path_schema = mk(
            r#"{"type":"object","properties":{"path":{"type":"string","description":"Absolute or relative file or directory path"}}}"#,
        );
        let required_path_schema = mk(
            r#"{"type":"object","properties":{"path":{"type":"string","description":"Absolute or relative file or directory path"}},"required":["path"]}"#,
        );
        let name_schema = mk(
            r#"{"type":"object","properties":{"symbol_name":{"type":"string","description":"Preferred symbol name parameter"},"name":{"type":"string","description":"Legacy alias for symbol_name"},"symbol":{"type":"string","description":"Legacy alias for symbol_name"},"exact":{"type":"boolean","description":"true=exact match, false=fuzzy (default: true)"}}}"#,
        );
        let sym_schema = mk(
            r#"{"type":"object","properties":{"symbol_name":{"type":"string","description":"Preferred symbol name parameter"},"name":{"type":"string","description":"Legacy alias for symbol_name"},"symbol":{"type":"string","description":"Legacy alias for symbol_name"}}}"#,
        );
        let query_schema = mk(
            r#"{"type":"object","properties":{"query":{"type":"string","description":"Natural language or keyword query"},"limit":{"type":"integer","description":"Max results (default: 10)"},"mode":{"type":"string","description":"bm25 | vector | hybrid (default: hybrid)"},"language":{"type":"string","description":"Filter by language: rust, python, typescript, …"}},"required":["query"]}"#,
        );
        let impact_schema = mk(
            r#"{"type":"object","properties":{"symbol_name":{"type":"string","description":"Preferred symbol name parameter"},"name":{"type":"string","description":"Legacy alias for symbol_name"},"symbol":{"type":"string","description":"Legacy alias for symbol_name"},"max_depth":{"type":"integer","description":"BFS depth (default: 5)"}}}"#,
        );
        let code_schema = mk(
            r#"{"type":"object","properties":{"operation":{"type":"string","description":"get_document_symbols | search_symbols | lookup_symbols | list_symbols | pattern_search | pattern_rewrite | edit_plan | search_codebase_map"},"path":{"type":"string","description":"Preferred file or directory path parameter"},"file_path":{"type":"string","description":"Legacy alias for path"},"symbol_name":{"type":"string","description":"Preferred symbol name parameter"},"name":{"type":"string","description":"Legacy alias for symbol_name"},"symbols":{"type":"array","items":{"type":"string"},"description":"Array of symbol names (lookup_symbols); comma-string also accepted"},"pattern":{"type":"string","description":"Regex or ast-grep pattern (pattern_search, pattern_rewrite)"},"query":{"type":"string","description":"Filter query (search_codebase_map)"},"language":{"type":"string","description":"Language filter for pattern_search"},"replacement":{"type":"string","description":"Replacement string (pattern_rewrite)"},"dry_run":{"type":"boolean","description":"Preview only, no writes (pattern_rewrite, default: true)"},"goal":{"type":"string","description":"Refactoring goal description (edit_plan)"},"include_source":{"type":"boolean","description":"Include source code in lookup_symbols (default: false)"}},"required":["operation"]}"#,
        );
        let mem_schema = mk(
            r#"{"type":"object","properties":{"content":{"type":"string","description":"Text to store"},"memory_type":{"type":"string","description":"note | decision | preference | conversation | status | doc"},"tags":{"type":"array","items":{"type":"string"},"description":"Tag list; comma-string also accepted"},"ttl":{"type":"string","description":"permanent | session | day | week | month"}},"required":["content"]}"#,
        );
        let recall_schema = mk(
            r#"{"type":"object","properties":{"query":{"type":"string","description":"What to search for in memories"},"limit":{"type":"integer","description":"Max results (default: 5)"},"memory_type":{"type":"string","description":"Filter by type: note, decision, …"},"tags":{"type":"string","description":"Filter by tag"}},"required":["query"]}"#,
        );
        let knowledge_schema = mk(
            r#"{"type":"object","properties":{"command":{"type":"string","description":"add | search | show | remove | update (omit to auto-detect from query)"},"name":{"type":"string","description":"Knowledge base name (add, remove, update)"},"query":{"type":"string","description":"Search query (search); also triggers search when command is omitted"},"value":{"type":"string","description":"Content or file/directory path to index (add)"},"limit":{"type":"integer","description":"Max results (search, default: 5)"}}}"#,
        );
        let ref_schema = mk(
            r#"{"type":"object","properties":{"ref_id":{"type":"string","description":"Reference ID returned by compact"}},"required":["ref_id"]}"#,
        );
        let commit_schema = mk(
            r#"{"type":"object","properties":{"query":{"type":"string","description":"Keywords or description to search commit messages"},"limit":{"type":"integer","description":"Max results"},"author":{"type":"string","description":"Filter by author name"}},"required":["query"]}"#,
        );
        let hist_schema = mk(
            r#"{"type":"object","properties":{"limit":{"type":"integer","description":"Number of commits to return (default: 20)"}}}"#,
        );

        vec![
            Tool::new("status",  "Show indexing state, graph stats, memory count, uptime", empty.clone()),
            Tool::new("health",  "Health check — returns healthy/unhealthy", empty.clone()),
            Tool::new("index",   "Index a codebase: builds symbol graph, BM25 index, and vector index. Args: path (required)", required_path_schema.clone()),
            Tool::new("search",  "Hybrid/vector/BM25 code search. Args: query (required), limit, mode (hybrid|vector|bm25), language", query_schema),
            Tool::new("find_symbol",  "Find where a symbol is defined. Args: symbol_name (preferred), name/symbol aliases, exact", name_schema),
            Tool::new("find_callers", "Who calls this function? Args: symbol_name (preferred), name/symbol aliases", sym_schema.clone()),
            Tool::new("find_callees", "What does this function call? Args: symbol_name (preferred), name/symbol aliases", sym_schema.clone()),
            Tool::new("explain",      "Natural-language symbol summary plus callers/callees/docstring. Args: symbol_name (preferred), name/symbol aliases", sym_schema.clone()),
            Tool::new("impact",       "Transitive blast radius of changing a symbol. Args: symbol_name (preferred), name/symbol aliases, max_depth", impact_schema),
            Tool::new("overview",     "Project overview: totals, languages, symbol types, top files/directories", empty.clone()),
            Tool::new("architecture", "Architectural layers, entry points, hub symbols by connectivity", empty.clone()),
            Tool::new("analyze",      "Code complexity and hotspots for a file or directory. Args: path", path_schema.clone()),
            Tool::new("focus",        "Per-symbol callers/callees for a file or directory. Args: path", path_schema.clone()),
            Tool::new("dead_code",    "Symbols with no callers (unreachable from entry points)", empty.clone()),
            Tool::new("circular_dependencies", "Detect circular import cycles", empty.clone()),
            Tool::new("test_coverage_map",     "Static heuristic test coverage estimate (not runtime coverage)", empty.clone()),
            Tool::new("code", "AST operations. Args: operation (required) — get_document_symbols(path), search_symbols(symbol_name), lookup_symbols(symbols:[]), list_symbols(path), pattern_search(pattern,path), pattern_rewrite(pattern,replacement,dry_run), edit_plan(goal), search_codebase_map(query,path)", code_schema),
            Tool::new("remember", "Store a memory/note. Args: content (required), memory_type, tags, ttl", mem_schema),
            Tool::new("recall",   "Search memories by meaning. Args: query (required), limit, memory_type, tags", recall_schema),
            Tool::new("tags",     "List all unique tags used in stored memories", empty.clone()),
            Tool::new("forget",   "Delete memories. Args: memory_id | tags | memory_type (at least one required)",
                mk(r#"{"type":"object","properties":{"memory_id":{"type":"string","description":"ID of a specific memory to delete"},"tags":{"type":"string","description":"Delete all memories with this tag"},"memory_type":{"type":"string","description":"Delete all memories of this type"}}}"#)),
            Tool::new("knowledge", "Index and search project docs/notes. Args: command (add|search|show|remove), name, query, value", knowledge_schema),
            Tool::new("compact",   "Archive session content and get a ref_id for later retrieval. Args: content (required)",
                mk(r#"{"type":"object","properties":{"content":{"type":"string","description":"Session content to archive"}},"required":["content"]}"#)),
            Tool::new("session_snapshot", "Show recent tool calls with arguments — useful after compaction", empty.clone()),
            Tool::new("restore",  "Project re-entry summary: graph size, path, recent session activity", empty.clone()),
            Tool::new("retrieve", "Fetch previously archived content by ref_id. Args: ref_id (required)", ref_schema),
            Tool::new("commit_search",  "Semantic search over git commit messages. Args: query (required), limit, author", commit_schema),
            Tool::new("commit_history", "Recent git commits with author and timestamp. Args: limit", hist_schema),
            Tool::new("repo_add",    "Register an additional repository for multi-repo analysis. Args: path", required_path_schema.clone()),
            Tool::new("repo_remove", "Unregister a repository. Args: path", required_path_schema),
            Tool::new("repo_status", "Show all registered repositories", empty.clone()),
            Tool::new("audit",        "Code quality audit report with recommendations", empty.clone()),
            Tool::new("docs_bundle",  "Generate Markdown docs bundle. Args: output_dir",
                mk(r#"{"type":"object","properties":{"output_dir":{"type":"string","description":"Output directory for generated docs (default: .contextro-docs)"}}}"#)),
            Tool::new("sidecar_export", "Export graph sidecar files alongside source. Args: path",
                mk(r#"{"type":"object","properties":{"path":{"type":"string","description":"Directory to write .graph.* sidecar files"}}}"#)),
            Tool::new("skill_prompt", "Return the agent bootstrap block for use in system prompts", empty.clone()),
            Tool::new("introspect",   "Find the right Contextro tool for a task. Args: query",
                mk(r#"{"type":"object","properties":{"query":{"type":"string","description":"Describe what you want to do"}}}"#)),
            Tool::new("refactor_check", "Pre-refactor analysis: definition + callers + callees + transitive impact + risk in one call. Args: symbol_name (required), max_depth",
                mk(r#"{"type":"object","properties":{"symbol_name":{"type":"string","description":"Symbol to analyze before refactoring"},"max_depth":{"type":"integer","description":"BFS depth for impact (default: 3)"}},"required":["symbol_name"]}"#)),
        ]
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
        // Find last complete JSON object boundary before limit
        let truncated = &output[..effective_budget];
        if let Some(pos) = truncated.rfind("},") {
            let budget_tokens = effective_budget / 4;
            return format!(
                "{}}}],\"truncated\":true,\"hint\":\"Response truncated to ~{} tokens. Use max_tokens for a different budget, or narrow your query.\"}}",
                &output[..pos], budget_tokens
            );
        }
        let budget_tokens = effective_budget / 4;
        return format!(
            "{}...[truncated to ~{} tokens. Narrow your query or set max_tokens.]",
            &output[..effective_budget],
            budget_tokens
        );
    }
    output
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

/// Recursively replace any string value that starts with `base/` with the relative path.
fn strip_absolute_paths(value: Value, base: &str) -> Value {
    let prefix = format!("{}/", base);
    match value {
        Value::String(s) => {
            if s.starts_with(&prefix) {
                Value::String(s[prefix.len()..].to_string())
            } else if s == base {
                Value::String(".".to_string())
            } else {
                Value::String(s)
            }
        }
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, strip_absolute_paths(v, base)))
                .collect(),
        ),
        Value::Array(arr) => Value::Array(
            arr.into_iter()
                .map(|v| strip_absolute_paths(v, base))
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
    if knowledge.show().into_iter().any(|(_, count)| count > 0) {
        return 0; // KB already has content; don't overwrite
    }
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
    let mut count = 0;
    for name in &candidates {
        let p = std::path::Path::new(root).join(name);
        if let Ok(content) = std::fs::read_to_string(&p) {
            if !content.trim().is_empty() {
                let chunks = knowledge.add(name, &content);
                if chunks > 0 {
                    count += 1;
                }
            }
        }
    }
    count
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
            instructions: Some("Contextro: code intelligence MCP server. 35 tools for search, graph analysis, memory, and git.".into()),
        }
    }

    fn list_tools(
        &self,
        _request: PaginatedRequestParam,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        // #2: Sort alphabetically for prompt cache hits
        // #10: Tool tiering via CTX_TOOL_TIER env var
        let mut tools = Self::tool_definitions();
        tools.sort_by(|a, b| a.name.cmp(&b.name));

        let tier = std::env::var("CTX_TOOL_TIER").unwrap_or_else(|_| "full".to_string());
        let core_tools: &[&str] = &[
            "code",
            "explain",
            "find_callers",
            "find_callees",
            "find_symbol",
            "health",
            "impact",
            "index",
            "search",
            "status",
        ];
        let standard_tools: &[&str] = &[
            "analyze",
            "architecture",
            "code",
            "commit_history",
            "commit_search",
            "dead_code",
            "explain",
            "find_callers",
            "find_callees",
            "find_symbol",
            "focus",
            "forget",
            "health",
            "impact",
            "index",
            "introspect",
            "knowledge",
            "overview",
            "recall",
            "remember",
            "search",
            "status",
        ];

        let filtered = match tier.as_str() {
            "core" => tools
                .into_iter()
                .filter(|t| core_tools.contains(&t.name.as_ref()))
                .collect(),
            "standard" => tools
                .into_iter()
                .filter(|t| standard_tools.contains(&t.name.as_ref()))
                .collect(),
            _ => tools, // "full" — all tools
        };

        std::future::ready(Ok(ListToolsResult {
            tools: filtered,
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
