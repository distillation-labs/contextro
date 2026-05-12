//! Graph-based tools: find_callers, find_callees, explain, impact.

use std::collections::{HashSet, VecDeque};
use std::path::Path;

use contextro_engines::graph::CodeGraph;
use serde_json::{json, Value};

/// Resolve `symbol_name` with a backward-compat fallback to the old `symbol` param name.
fn get_symbol_name<'a>(args: &'a Value) -> &'a str {
    args.get("symbol_name")
        .or_else(|| args.get("symbol"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

pub fn handle_find_callers(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let name = get_symbol_name(args);
    if name.is_empty() {
        return json!({"error": "Missing required parameter: symbol_name"});
    }

    let matches = resolve_symbol(name, graph);
    if matches.is_empty() {
        return json!({"error": format!("Symbol '{}' not found.", name)});
    }

    let mut callers = Vec::new();
    for node in &matches {
        for caller in graph.get_callers(&node.id) {
            let fp = relativize(&caller.location.file_path, codebase);
            callers.push(format!("{} ({}:{})", caller.name, fp, caller.location.start_line));
        }
    }

    let mut result = json!({"symbol": name, "callers": callers, "total": callers.len()});
    if callers.is_empty() {
        let is_type = matches.iter().any(|n| {
            matches!(n.node_type,
                contextro_core::NodeType::Class |
                contextro_core::NodeType::Interface |
                contextro_core::NodeType::Enum)
        });
        if is_type {
            result["hint"] = json!(
                "This is a type (struct/class/enum) — types have no call-graph edges. \
                 Try querying a method or constructor: find_callers('new') or search() for usage."
            );
        }
    }
    result
}

pub fn handle_find_callees(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let name = get_symbol_name(args);
    if name.is_empty() {
        return json!({"error": "Missing required parameter: symbol_name"});
    }

    let matches = resolve_symbol(name, graph);
    if matches.is_empty() {
        return json!({"error": format!("Symbol '{}' not found.", name)});
    }

    let mut callees = Vec::new();
    for node in &matches {
        for callee in graph.get_callees(&node.id) {
            let fp = relativize(&callee.location.file_path, codebase);
            callees.push(format!("{} ({}:{})", callee.name, fp, callee.location.start_line));
        }
    }

    let mut result = json!({"symbol": name, "callees": callees, "total": callees.len()});
    if callees.is_empty() {
        let is_type = matches.iter().any(|n| {
            matches!(n.node_type,
                contextro_core::NodeType::Class |
                contextro_core::NodeType::Interface |
                contextro_core::NodeType::Enum)
        });
        if is_type {
            result["hint"] = json!(
                "This is a type (struct/class/enum) — types have no call-graph edges. \
                 Try querying its methods directly by name."
            );
        }
    }
    result
}

pub fn handle_explain(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let name = get_symbol_name(args);
    if name.is_empty() {
        return json!({"error": "Missing required parameter: symbol_name"});
    }

    let matches = resolve_symbol(name, graph);
    if matches.is_empty() {
        return json!({"error": format!("Symbol '{}' not found.", name)});
    }

    let node = &matches[0];
    let callers: Vec<String> = graph
        .get_callers(&node.id)
        .iter()
        .take(10)
        .map(|c| format!("{} ({}:{})", c.name, relativize(&c.location.file_path, codebase), c.location.start_line))
        .collect();
    let callees: Vec<String> = graph
        .get_callees(&node.id)
        .iter()
        .take(10)
        .map(|c| format!("{} ({}:{})", c.name, relativize(&c.location.file_path, codebase), c.location.start_line))
        .collect();

    json!({
        "name": node.name,
        "type": node.node_type.to_string(),
        "file": relativize(&node.location.file_path, codebase),
        "line": node.location.start_line,
        "language": node.language,
        "docstring": node.docstring,
        "callers": callers,
        "callees": callees,
    })
}

pub fn handle_impact(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let name = get_symbol_name(args);
    let max_depth = args.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

    if name.is_empty() {
        return json!({"error": "Missing required parameter: symbol_name"});
    }

    let matches = resolve_symbol(name, graph);
    if matches.is_empty() {
        return json!({"error": format!("Symbol '{}' not found.", name)});
    }

    // BFS transitive callers
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    let mut impacted = Vec::new();

    for node in &matches {
        queue.push_back((node.id.clone(), 0));
        visited.insert(node.id.clone());
    }

    while let Some((node_id, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        for caller in graph.get_callers(&node_id) {
            if visited.insert(caller.id.clone()) {
                let fp = relativize(&caller.location.file_path, codebase);
                impacted.push(json!({
                    "name": caller.name,
                    "file": fp,
                    "line": caller.location.start_line,
                    "depth": depth + 1,
                }));
                queue.push_back((caller.id.clone(), depth + 1));
            }
        }
    }

    let mut result = json!({
        "symbol": name,
        "max_depth": max_depth,
        "impacted": impacted,
        "total_impacted": impacted.len(),
    });

    // Hint for entry points: 0 transitive callers means nothing depends on this symbol,
    // which is expected for top-level entry points (main, CLI handlers, etc.)
    if impacted.is_empty() {
        let (in_degree, _) = graph.get_node_degree(&matches[0].id);
        if in_degree == 0 {
            result["hint"] = json!(
                "0 callers found — this symbol is a root entry point (nothing calls it in the parsed AST). \
                 It is safe to change its signature, but check external callers (CLI, tests, MCP handlers) manually."
            );
        }
    }

    result
}

fn relativize(filepath: &str, codebase: Option<&str>) -> String {
    match codebase {
        Some(base) => Path::new(filepath)
            .strip_prefix(base)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| filepath.to_string()),
        None => filepath.to_string(),
    }
}

/// Resolve a symbol name: exact match first, fall back to fuzzy.
/// Ranks candidates by call frequency so the most-connected symbol wins on name collision.
fn resolve_symbol(name: &str, graph: &CodeGraph) -> Vec<contextro_core::UniversalNode> {
    let exact = graph.find_nodes_by_name(name, true);
    if !exact.is_empty() {
        let mut ranked = exact;
        ranked.sort_by_key(|n| {
            let (in_d, out_d) = graph.get_node_degree(&n.id);
            std::cmp::Reverse(in_d + out_d)
        });
        return ranked;
    }
    let mut fuzzy = graph.find_nodes_by_name(name, false);
    fuzzy.sort_by_key(|n| {
        let (in_d, out_d) = graph.get_node_degree(&n.id);
        std::cmp::Reverse(in_d + out_d)
    });
    fuzzy.into_iter().take(5).collect()
}
