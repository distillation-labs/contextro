use super::*;

use super::response_utils::strip_codebase;
use super::support::resolve_refactor_targets;

impl ContextroServer {
    pub(crate) fn handle_find_symbol(&self, args: &Value) -> Value {
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

        let total_matches = matches.len();
        let symbols: Vec<Value> = matches
            .iter()
            .take(20)
            .map(|node| {
                let fp = cb
                    .as_ref()
                    .map(|b| {
                        std::path::Path::new(&node.location.file_path)
                            .strip_prefix(b)
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|_| node.location.file_path.clone())
                    })
                    .unwrap_or_else(|| node.location.file_path.clone());
                json!({
                    "name": node.name,
                    "file": fp,
                    "line": node.location.start_line,
                    "type": node.node_type.to_string()
                })
            })
            .collect();

        let mut response = json!({"symbols": symbols});
        if total_matches > response["symbols"].as_array().map_or(0, Vec::len) {
            response["total"] = json!(total_matches);
            response["truncated"] = json!(true);
        }
        response
    }

    /// #6: Composite tool — find_symbol + callers + impact + explain in one call.
    pub(crate) fn handle_refactor_check(&self, args: &Value) -> Value {
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
    pub(crate) fn handle_completion_check(&self, args: &Value) -> Value {
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
