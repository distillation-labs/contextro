use std::collections::HashMap;

use contextro_core::NodeType;
use contextro_engines::graph::CodeGraph;
use serde_json::{json, Value};

use super::{
    is_generic_symbol_name, is_probable_public_api, is_pytest_fixture, is_test_file,
    parse_path_filters, path_matches_cached, resolve_existing_path, strip_base,
};

pub fn handle_dead_code(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let snapshot = graph.snapshot();
    let nodes = snapshot.nodes();
    let mut dead: Vec<Value> = Vec::new();
    let mut file_cache: HashMap<String, Option<String>> = HashMap::new();
    let mut normalized_paths = HashMap::new();
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    let include_public_api = args
        .get("include_public_api")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let include_tests = args
        .get("include_tests")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let path_filter = match args.get("path").and_then(|v| v.as_str()) {
        Some(path) if !path.is_empty() => {
            let abs_path = match resolve_existing_path(path, codebase) {
                Ok(path) => path,
                Err(error) => return error,
            };
            Some((abs_path.clone(), abs_path.is_dir()))
        }
        _ => None,
    };
    let excluded_paths = match parse_path_filters(args.get("exclude_paths"), codebase) {
        Ok(paths) => paths,
        Err(error) => return error,
    };
    let mut skipped_public_api = 0usize;
    let mut skipped_tests = 0usize;
    let mut skipped_excluded = 0usize;

    for node in nodes {
        // Skip classes and variables — focus on functions/methods
        if node.node_type != NodeType::Function {
            continue;
        }
        if let Some((target_path, is_dir)) = &path_filter {
            if !path_matches_cached(
                &node.location.file_path,
                target_path,
                *is_dir,
                &mut normalized_paths,
            ) {
                continue;
            }
        }
        if excluded_paths.iter().any(|(target_path, is_dir)| {
            path_matches_cached(
                &node.location.file_path,
                target_path,
                *is_dir,
                &mut normalized_paths,
            )
        }) {
            skipped_excluded += 1;
            continue;
        }
        if !include_tests && is_test_file(&node.location.file_path) {
            skipped_tests += 1;
            continue;
        }
        let (in_degree, _) = snapshot.degree(&node.id);
        if in_degree == 0 {
            let name_lower = node.name.to_lowercase();
            let is_entry = name_lower == "main"
                || name_lower.starts_with("test_")
                || name_lower.starts_with("__")
                || name_lower == "setup"
                || name_lower == "teardown";
            let is_noise = is_generic_symbol_name(&node.name);
            if !include_public_api && is_probable_public_api(node) {
                skipped_public_api += 1;
                continue;
            }
            if !is_entry && !is_noise && !is_pytest_fixture(node, &mut file_cache) {
                dead.push(json!({
                    "name": node.name,
                    "file": strip_base(&node.location.file_path, codebase),
                    "line": node.location.start_line,
                    "type": node.node_type.to_string(),
                }));
            }
        }
    }
    dead.sort_by(|a, b| a["file"].as_str().cmp(&b["file"].as_str()));
    dead.truncate(limit);

    let mut result = json!({
        "dead_symbols": dead,
        "total": dead.len(),
        "limit": limit,
        "note": "Static heuristic: zero parsed callers after filtering entry points, test files, and public API surface by default",
    });
    if skipped_public_api > 0 {
        result["skipped_public_api"] = json!(skipped_public_api);
    }
    if skipped_tests > 0 {
        result["skipped_tests"] = json!(skipped_tests);
    }
    if skipped_excluded > 0 {
        result["skipped_excluded"] = json!(skipped_excluded);
    }
    if let Some((target_path, _)) = path_filter {
        result["path"] = json!(strip_base(&target_path.to_string_lossy(), codebase));
    }
    if !excluded_paths.is_empty() {
        result["excluded_paths"] = json!(excluded_paths
            .iter()
            .map(|(path, _)| strip_base(&path.to_string_lossy(), codebase))
            .collect::<Vec<_>>());
    }
    result
}
