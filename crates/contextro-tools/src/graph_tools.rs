//! Graph-based tools: find_callers, find_callees, explain, impact.

use std::collections::{HashSet, VecDeque};
use std::path::Path;

use contextro_engines::graph::CodeGraph;
use serde_json::{json, Value};

/// Resolve the preferred `symbol_name` plus backward-compatible aliases.
fn get_symbol_name(args: &Value) -> &str {
    args.get("symbol_name")
        .or_else(|| args.get("name"))
        .or_else(|| args.get("symbol"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

pub fn handle_find_callers(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let name = get_symbol_name(args);
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
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
            callers.push(format!(
                "{} ({}:{})",
                caller.name, fp, caller.location.start_line
            ));
            if callers.len() >= limit {
                break;
            }
        }
        if callers.len() >= limit {
            break;
        }
    }

    let mut result =
        json!({"symbol": name, "callers": callers, "total": callers.len(), "limit": limit});
    if callers.is_empty() {
        let is_type = matches.iter().any(|n| {
            matches!(
                n.node_type,
                contextro_core::NodeType::Class
                    | contextro_core::NodeType::Interface
                    | contextro_core::NodeType::Enum
            )
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
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
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
            callees.push(format!(
                "{} ({}:{})",
                callee.name, fp, callee.location.start_line
            ));
            if callees.len() >= limit {
                break;
            }
        }
        if callees.len() >= limit {
            break;
        }
    }

    let mut result =
        json!({"symbol": name, "callees": callees, "total": callees.len(), "limit": limit});
    if callees.is_empty() {
        let is_type = matches.iter().any(|n| {
            matches!(
                n.node_type,
                contextro_core::NodeType::Class
                    | contextro_core::NodeType::Interface
                    | contextro_core::NodeType::Enum
            )
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
    let all_callers = graph.get_callers(&node.id);
    let all_callees = graph.get_callees(&node.id);
    let callers: Vec<String> = all_callers
        .iter()
        .take(10)
        .map(|c| {
            format!(
                "{} ({}:{})",
                c.name,
                relativize(&c.location.file_path, codebase),
                c.location.start_line
            )
        })
        .collect();
    let callees: Vec<String> = all_callees
        .iter()
        .take(10)
        .map(|c| {
            // #4: Type-qualified name if parent is available
            let display = if let Some(ref parent) = c.parent {
                format!("{}.{}", parent, c.name)
            } else {
                c.name.clone()
            };
            format!(
                "{} ({}:{})",
                display,
                relativize(&c.location.file_path, codebase),
                c.location.start_line
            )
        })
        .collect();
    let summary = build_explanation_summary(node, all_callers.len(), all_callees.len(), codebase);

    json!({
        "name": node.name,
        "type": node.node_type.to_string(),
        "file": relativize(&node.location.file_path, codebase),
        "line": node.location.start_line,
        "language": node.language,
        "docstring": node.docstring,
        "summary": summary,
        "callers_count": all_callers.len(),
        "callees_count": all_callees.len(),
        "callers": callers,
        "callees": callees,
    })
}

pub fn handle_impact(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    const DEFAULT_IMPACT_DEPTH: usize = 5;
    let name = get_symbol_name(args);
    let requested_depth = args.get("max_depth").and_then(|v| v.as_u64());
    let max_depth = requested_depth.unwrap_or(DEFAULT_IMPACT_DEPTH as u64) as usize;

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
        "default_depth": DEFAULT_IMPACT_DEPTH,
        "impacted": impacted,
        "total": impacted.len(),
        "total_impacted": impacted.len(),
    });

    if let Some(explicit_depth) = requested_depth {
        if explicit_depth as usize != DEFAULT_IMPACT_DEPTH {
            result["depth_hint"] = json!(format!(
                "Explicit max_depth={} overrides the default depth of {}. Smaller depths intentionally return a narrower impact set.",
                explicit_depth,
                DEFAULT_IMPACT_DEPTH
            ));
        }
    }

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

fn build_explanation_summary(
    node: &contextro_core::UniversalNode,
    callers_count: usize,
    callees_count: usize,
    codebase: Option<&str>,
) -> String {
    let location = format!(
        "{}:{}",
        relativize(&node.location.file_path, codebase),
        node.location.start_line
    );
    let doc = node
        .docstring
        .as_deref()
        .map(str::trim)
        .filter(|doc| !doc.is_empty())
        .map(|doc| format!(" {doc}"))
        .unwrap_or_default();
    format!(
        "{} is a {} defined at {}. It currently has {} caller(s) and {} callee(s).{}",
        node.name, node.node_type, location, callers_count, callees_count, doc
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use contextro_core::graph::{
        RelationshipType, UniversalLocation, UniversalNode, UniversalRelationship,
    };
    use contextro_core::NodeType;

    fn sample_node(id: &str, name: &str, file: &str, line: u32) -> UniversalNode {
        UniversalNode {
            id: id.into(),
            name: name.into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: file.into(),
                start_line: line,
                end_line: line,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        }
    }

    #[test]
    fn test_impact_results_are_monotonic_with_depth() {
        let graph = CodeGraph::new();
        let file = "/tmp/repo/src/session.py";

        for (id, name, line) in [
            ("leaf", "BrowserSession", 10_u32),
            ("mid", "create_browser_session", 20_u32),
            ("root", "start_app", 30_u32),
        ] {
            graph.add_node(UniversalNode {
                id: id.into(),
                name: name.into(),
                node_type: NodeType::Function,
                location: UniversalLocation {
                    file_path: file.into(),
                    start_line: line,
                    end_line: line,
                    start_column: 0,
                    end_column: 0,
                    language: "python".into(),
                },
                language: "python".into(),
                ..Default::default()
            });
        }

        graph.add_relationship(UniversalRelationship {
            id: "rel-mid-leaf".into(),
            source_id: "mid".into(),
            target_id: "leaf".into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });
        graph.add_relationship(UniversalRelationship {
            id: "rel-root-mid".into(),
            source_id: "root".into(),
            target_id: "mid".into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });

        let depth_one = handle_impact(
            &json!({"symbol_name":"BrowserSession","max_depth":1}),
            &graph,
            Some("/tmp/repo"),
        );
        let depth_three = handle_impact(
            &json!({"symbol_name":"BrowserSession","max_depth":3}),
            &graph,
            Some("/tmp/repo"),
        );

        assert_eq!(depth_one["total"], 1);
        assert_eq!(depth_one["total_impacted"], 1);
        assert_eq!(depth_three["total"], 2);
        assert_eq!(depth_three["total_impacted"], 2);

        let shallow = depth_one["impacted"]
            .as_array()
            .expect("depth-one impacted list")
            .iter()
            .filter_map(|entry| entry["name"].as_str())
            .collect::<Vec<_>>();
        let deep = depth_three["impacted"]
            .as_array()
            .expect("depth-three impacted list")
            .iter()
            .filter_map(|entry| entry["name"].as_str())
            .collect::<Vec<_>>();

        assert!(deep.len() >= shallow.len());
        for name in shallow {
            assert!(deep.contains(&name));
        }
    }

    #[test]
    fn test_impact_reports_default_depth_and_explicit_depth_hint() {
        let graph = CodeGraph::new();
        graph.add_node(UniversalNode {
            id: "leaf".into(),
            name: "BrowserSession".into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: "/tmp/repo/src/session.py".into(),
                start_line: 10,
                end_line: 10,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            ..Default::default()
        });

        let default_result = handle_impact(
            &json!({"symbol_name":"BrowserSession"}),
            &graph,
            Some("/tmp/repo"),
        );
        let explicit_result = handle_impact(
            &json!({"symbol_name":"BrowserSession","max_depth":3}),
            &graph,
            Some("/tmp/repo"),
        );

        assert_eq!(default_result["default_depth"], 5);
        assert_eq!(default_result["total"], 0);
        assert!(default_result.get("depth_hint").is_none());
        assert_eq!(explicit_result["default_depth"], 5);
        assert_eq!(explicit_result["total"], 0);
        assert!(explicit_result["depth_hint"]
            .as_str()
            .unwrap_or("")
            .contains("narrower impact set"));
    }

    #[test]
    fn test_find_callers_respects_limit() {
        let graph = CodeGraph::new();
        let file = "/tmp/repo/src/lib.rs";
        graph.add_node(sample_node("target", "dispatch", file, 10));
        graph.add_node(sample_node("caller-1", "call_one", file, 20));
        graph.add_node(sample_node("caller-2", "call_two", file, 30));
        graph.add_relationship(UniversalRelationship {
            id: "rel-1".into(),
            source_id: "caller-1".into(),
            target_id: "target".into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });
        graph.add_relationship(UniversalRelationship {
            id: "rel-2".into(),
            source_id: "caller-2".into(),
            target_id: "target".into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });

        let result = handle_find_callers(
            &json!({"symbol_name":"dispatch","limit":1}),
            &graph,
            Some("/tmp/repo"),
        );

        assert_eq!(result["limit"], 1);
        assert_eq!(result["total"], 1);
    }

    #[test]
    fn test_find_callees_respects_limit() {
        let graph = CodeGraph::new();
        let file = "/tmp/repo/src/lib.rs";
        graph.add_node(sample_node("target", "dispatch", file, 10));
        graph.add_node(sample_node("callee-1", "handle_one", file, 20));
        graph.add_node(sample_node("callee-2", "handle_two", file, 30));
        graph.add_relationship(UniversalRelationship {
            id: "rel-1".into(),
            source_id: "target".into(),
            target_id: "callee-1".into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });
        graph.add_relationship(UniversalRelationship {
            id: "rel-2".into(),
            source_id: "target".into(),
            target_id: "callee-2".into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });

        let result = handle_find_callees(
            &json!({"symbol_name":"dispatch","limit":1}),
            &graph,
            Some("/tmp/repo"),
        );

        assert_eq!(result["limit"], 1);
        assert_eq!(result["total"], 1);
    }
}
