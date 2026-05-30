use super::*;

use super::response_utils::{
    format_response, response_needs_path_stripping, sanitize_tool_args, strip_codebase,
    strip_response_paths, summarize_tool_call, take_chars,
};
use super::support::{
    cached_tool_result, edit_distance, response_cache_key, strip_render_only_args,
};

impl ContextroServer {
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
            "code" => contextro_tools::code::handle_code(&args, &s.graph, Some(&s.query_cache), cb),
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
}
