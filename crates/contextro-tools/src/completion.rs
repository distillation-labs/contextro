//! Completion-check helpers for verifying refactor completeness.
//!
//! These tools let coding agents answer "Did I actually finish this refactor?"
//! by comparing the code graph against the set of files they claim to have changed.

use std::collections::BTreeSet;
use std::path::Path;

use contextro_core::UniversalNode;
use contextro_engines::graph::CodeGraph;
use serde_json::{json, Value};

pub fn handle_completion_check(
    args: &Value,
    graph: &CodeGraph,
    codebase: Option<&str>,
) -> Value {
    let name = args
        .get("symbol_name")
        .or_else(|| args.get("name"))
        .or_else(|| args.get("symbol"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if name.is_empty() {
        return json!({"error": "Missing required parameter: symbol_name"});
    }

    let claim = args
        .get("claim")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if claim.is_empty() {
        return json!({"error": "Missing required parameter: claim"});
    }

    let changed_files: Vec<String> = args
        .get("changed_files")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if changed_files.is_empty() {
        return json!({"error": "Missing required parameter: changed_files"});
    }

    let max_depth = args
        .get("max_depth")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    match claim {
        "all_callers_updated" => {
            check_all_callers_updated(name, &changed_files, graph, codebase, max_depth)
        }
        _ => json!({
            "error": format!("Unknown claim '{}'. Supported: all_callers_updated", claim),
            "hint": "Use claim='all_callers_updated' to verify that a rename/signature change updated every direct caller."
        }),
    }
}

fn check_all_callers_updated(
    symbol_name: &str,
    changed_files: &[String],
    graph: &CodeGraph,
    codebase: Option<&str>,
    _max_depth: usize,
) -> Value {
    let matches = resolve_symbol_for_completion(symbol_name, graph);
    if matches.is_empty() {
        return json!({
            "claim": "all_callers_updated",
            "symbol": symbol_name,
            "status": "unknown",
            "confidence": "low",
            "hint": format!("Symbol '{}' not found in the current index. Was the symbol renamed or deleted? Try search() or find_symbol() to locate it.", symbol_name),
        });
    }

    let node = &matches[0];
    let definition_file = relativize(&node.location.file_path, codebase);
    let expected_callers = caller_files(node, graph, codebase);
    let changed_files = normalize_changed_files(changed_files, codebase);
    let changed_set: BTreeSet<&String> = changed_files.iter().collect();

    let covered_callers: Vec<String> = expected_callers
        .iter()
        .filter(|file| changed_set.contains(file))
        .cloned()
        .collect();
    let missing_callers: Vec<String> = expected_callers
        .iter()
        .filter(|file| !changed_set.contains(file))
        .cloned()
        .collect();

    let relevant_files: BTreeSet<String> = expected_callers
        .iter()
        .cloned()
        .chain(std::iter::once(definition_file.clone()))
        .collect();
    let extra_changed_files: Vec<String> = changed_files
        .iter()
        .filter(|file| !relevant_files.contains(*file))
        .cloned()
        .collect();

    let coverage_ratio = if expected_callers.is_empty() {
        1.0
    } else {
        covered_callers.len() as f64 / expected_callers.len() as f64
    };

    let confidence = match (matches.len(), expected_callers.len()) {
        (count, _) if count > 1 => "medium",
        (_, 0) => "low",
        _ => "high",
    };

    let status = if missing_callers.is_empty()
        && matches.len() == 1
        && changed_set.contains(&definition_file)
    {
        "complete"
    } else if missing_callers.is_empty() {
        "needs_review"
    } else {
        "incomplete"
    };

    let mut response = json!({
        "claim": "all_callers_updated",
        "symbol": symbol_name,
        "type": node.node_type.to_string(),
        "definition_file": definition_file,
        "definition_line": node.location.start_line,
        "changed_files": changed_files,
        "expected_callers": expected_callers,
        "covered_callers": covered_callers,
        "missing_callers": missing_callers,
        "extra_changed_files": extra_changed_files,
        "callers_considered": relevant_files.len().saturating_sub(1),
        "coverage_ratio": coverage_ratio,
        "status": status,
        "confidence": confidence,
    });

    if matches.len() > 1 {
        response["ambiguity"] = json!({
            "candidates": matches.len(),
            "hint": "Multiple exact symbol matches exist. Results use the most-connected candidate. Verify the definition file to ensure this is the intended symbol.",
        });
    }
    if expected_callers.is_empty() {
        response["hint"] = json!(
            "No direct caller files were found in the current graph. This may be a root symbol or a parser-coverage gap. Consider using find_callers() to double-check."
        );
    }
    if !extra_changed_files.is_empty() {
        response["warning"] = json!({
            "extra_changed_files": extra_changed_files,
            "hint": "These changed files do not appear in the expected caller or definition files. Verify they are intentionally included.",
        });
    }
    if matches.len() == 1
        && changed_set.contains(&definition_file)
        && !missing_callers.is_empty()
    {
        response["hint"] = json!(
            "Some expected caller files are missing from the changed set. Re-check your diff to ensure all callers were updated."
        );
    }

    response
}

fn caller_files(
    node: &UniversalNode,
    graph: &CodeGraph,
    codebase: Option<&str>,
) -> Vec<String> {
    let mut files = BTreeSet::new();
    for caller in graph.get_callers(&node.id) {
        files.insert(relativize(&caller.location.file_path, codebase));
    }
    files.into_iter().collect()
}

fn normalize_changed_files(changed_files: &[String], codebase: Option<&str>) -> Vec<String> {
    let mut normalized = BTreeSet::new();
    for file in changed_files {
        let cleaned = normalize_path_like(file, codebase);
        if !cleaned.is_empty() {
            normalized.insert(cleaned);
        }
    }
    normalized.into_iter().collect()
}

fn normalize_path_like(path: &str, codebase: Option<&str>) -> String {
    let path = path.trim();
    if path.is_empty() {
        return String::new();
    }

    if let Some(base) = codebase {
        if let Ok(relative) = Path::new(path).strip_prefix(base) {
            return relative.to_string_lossy().to_string();
        }
    }

    path.strip_prefix("./").unwrap_or(path).replace('\\', "/")
}

fn relativize(filepath: &str, codebase: Option<&str>) -> String {
    match codebase {
        Some(base) => Path::new(filepath)
            .strip_prefix(base)
            .map(|relative| relative.to_string_lossy().to_string())
            .unwrap_or_else(|_| filepath.to_string()),
        None => filepath.to_string(),
    }
}

fn resolve_symbol_for_completion(
    name: &str,
    graph: &CodeGraph,
) -> Vec<UniversalNode> {
    let exact = graph.find_nodes_by_name(name, true);
    if !exact.is_empty() {
        let mut ranked = exact;
        ranked.sort_by_key(|node| {
            let (in_degree, out_degree) = graph.get_node_degree(&node.id);
            std::cmp::Reverse(in_degree + out_degree)
        });
        return ranked;
    }

    let mut fuzzy = graph.find_nodes_by_name(name, false);
    fuzzy.sort_by_key(|node| {
        let (in_degree, out_degree) = graph.get_node_degree(&node.id);
        std::cmp::Reverse(in_degree + out_degree)
    });
    fuzzy.into_iter().take(5).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use contextro_core::graph::{
        RelationshipType, UniversalLocation, UniversalNode, UniversalRelationship,
    };
    use contextro_core::NodeType;

    fn add_function(graph: &CodeGraph, id: &str, name: &str, file: &str, line: u32) {
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
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        });
    }

    #[test]
    fn completion_check_reports_missing_callers() {
        let graph = CodeGraph::new();
        add_function(&graph, "target", "do_work", "/repo/src/target.rs", 10);
        add_function(&graph, "caller_a", "use_work_a", "/repo/src/a.rs", 20);
        add_function(&graph, "caller_b", "use_work_b", "/repo/src/b.rs", 30);

        graph.add_relationship(UniversalRelationship {
            id: "r1".into(),
            source_id: "caller_a".into(),
            target_id: "target".into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });
        graph.add_relationship(UniversalRelationship {
            id: "r2".into(),
            source_id: "caller_b".into(),
            target_id: "target".into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });

        let result = handle_completion_check(
            &json!({
                "claim": "all_callers_updated",
                "symbol_name": "do_work",
                "changed_files": ["src/target.rs", "src/a.rs"],
            }),
            &graph,
            Some("/repo"),
        );

        assert_eq!(result["status"], "incomplete");
        assert_eq!(result["missing_callers"], json!(["src/b.rs"]));
        assert_eq!(result["coverage_ratio"], 0.5);
    }

    #[test]
    fn completion_check_accepts_complete_direct_caller_set() {
        let graph = CodeGraph::new();
        add_function(&graph, "target", "rename_me", "/repo/src/lib.rs", 10);
        add_function(&graph, "caller", "wrap_rename", "/repo/src/wrap.rs", 20);

        graph.add_relationship(UniversalRelationship {
            id: "r1".into(),
            source_id: "caller".into(),
            target_id: "target".into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });

        let result = handle_completion_check(
            &json!({
                "claim": "all_callers_updated",
                "symbol_name": "rename_me",
                "changed_files": ["/repo/src/lib.rs", "/repo/src/wrap.rs"],
            }),
            &graph,
            Some("/repo"),
        );

        assert_eq!(result["status"], "complete");
        assert_eq!(result["missing_callers"], json!([]));
        assert_eq!(result["coverage_ratio"], 1.0);
    }

    #[test]
    fn completion_check_with_unknown_symbol_returns_unknown_status() {
        let graph = CodeGraph::new();
        add_function(&graph, "target", "exists", "/repo/src/lib.rs", 10);

        let result = handle_completion_check(
            &json!({
                "claim": "all_callers_updated",
                "symbol_name": "nonexistent",
                "changed_files": ["src/a.rs"],
            }),
            &graph,
            Some("/repo"),
        );

        assert_eq!(result["status"], "unknown");
        assert_eq!(result["confidence"], "low");
    }
}
