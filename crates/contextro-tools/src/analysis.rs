//! Analysis tools: overview, architecture, analyze, focus, dead_code, circular_dependencies, test_coverage_map.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use contextro_engines::graph::CodeGraph;
use contextro_core::NodeType;
use serde_json::{json, Value};

pub fn handle_overview(graph: &CodeGraph, codebase: Option<&str>, total_chunks: usize) -> Value {
    let node_count = graph.node_count();
    let rel_count = graph.relationship_count();
    json!({
        "codebase_path": codebase,
        "total_symbols": node_count,
        "total_relationships": rel_count,
        "vector_chunks": total_chunks,
    })
}

pub fn handle_architecture(graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let nodes = graph.find_nodes_by_name("", false);
    let mut scored: Vec<(String, String, usize)> = nodes.iter().map(|n| {
        let (in_d, out_d) = graph.get_node_degree(&n.id);
        (n.name.clone(), n.location.file_path.clone(), in_d + out_d)
    }).collect();
    scored.sort_by(|a, b| b.2.cmp(&a.2));

    let hubs: Vec<Value> = scored.iter().take(10).map(|(name, file, degree)| {
        let fp = strip_base(file, codebase);
        json!({"name": name, "file": fp, "degree": degree})
    }).collect();

    json!({"hub_symbols": hubs, "total_nodes": graph.node_count(), "total_edges": graph.relationship_count()})
}

pub fn handle_analyze(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let _path = args.get("path").and_then(|v| v.as_str());
    let nodes = graph.find_nodes_by_name("", false);
    let mut complex_fns: Vec<Value> = Vec::new();
    let mut file_sizes: HashMap<String, usize> = HashMap::new();

    for node in &nodes {
        *file_sizes.entry(node.location.file_path.clone()).or_default() += 1;
        let (in_d, out_d) = graph.get_node_degree(&node.id);
        if in_d + out_d > 5 {
            complex_fns.push(json!({"name": node.name, "file": strip_base(&node.location.file_path, codebase), "connections": in_d + out_d}));
        }
    }
    complex_fns.sort_by(|a, b| b["connections"].as_u64().cmp(&a["connections"].as_u64()));
    complex_fns.truncate(10);

    let mut large_files: Vec<Value> = file_sizes.iter()
        .filter(|(_, count)| **count > 10)
        .map(|(file, count)| json!({"file": strip_base(file, codebase), "symbols": count}))
        .collect();
    large_files.sort_by(|a, b| b["symbols"].as_u64().cmp(&a["symbols"].as_u64()));

    json!({"high_connectivity_symbols": complex_fns, "large_files": large_files, "total_symbols": nodes.len()})
}

/// Low-token context slice for a single file.
pub fn handle_focus(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return json!({"error": "Missing required parameter: path"});
    }

    let abs_path = if Path::new(path).is_absolute() {
        path.to_string()
    } else {
        codebase.map(|b| format!("{}/{}", b, path)).unwrap_or_else(|| path.to_string())
    };

    // Find symbols in this file from the graph
    let nodes = graph.find_nodes_by_name("", false);
    let file_symbols: Vec<Value> = nodes.iter()
        .filter(|n| n.location.file_path == abs_path || n.location.file_path.ends_with(path))
        .map(|n| {
            let (in_d, out_d) = graph.get_node_degree(&n.id);
            json!({"name": n.name, "type": n.node_type.to_string(), "line": n.location.start_line, "callers": in_d, "callees": out_d})
        })
        .collect();

    // Read first few lines for overview
    let preview = std::fs::read_to_string(&abs_path)
        .map(|s| s.lines().take(5).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default();

    json!({
        "file": strip_base(&abs_path, codebase),
        "symbols": file_symbols,
        "total_symbols": file_symbols.len(),
        "preview": preview,
    })
}

/// Dead code analysis: find symbols with zero callers that aren't entry points.
pub fn handle_dead_code(graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let nodes = graph.find_nodes_by_name("", false);
    let mut dead: Vec<Value> = Vec::new();

    for node in &nodes {
        // Skip classes and variables — focus on functions/methods
        if node.node_type != NodeType::Function {
            continue;
        }
        let (in_degree, _) = graph.get_node_degree(&node.id);
        if in_degree == 0 {
            // Check if it's likely an entry point (main, test_, __init__, etc.)
            let name_lower = node.name.to_lowercase();
            let is_entry = name_lower == "main"
                || name_lower.starts_with("test_")
                || name_lower.starts_with("__")
                || name_lower == "setup"
                || name_lower == "teardown";
            if !is_entry {
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
    dead.truncate(50);

    json!({"dead_symbols": dead, "total": dead.len(), "note": "Symbols with zero callers that aren't entry points"})
}

/// Circular dependency detection using Tarjan's SCC at file level.
pub fn handle_circular_dependencies(graph: &CodeGraph, codebase: Option<&str>) -> Value {
    // Build file-level dependency graph
    let nodes = graph.find_nodes_by_name("", false);
    let mut file_deps: HashMap<String, HashSet<String>> = HashMap::new();

    for node in &nodes {
        let callees = graph.get_callees(&node.id);
        for callee in &callees {
            let callee_nodes = graph.find_nodes_by_name(&callee.name, true);
            for cn in &callee_nodes {
                if cn.location.file_path != node.location.file_path {
                    file_deps.entry(node.location.file_path.clone())
                        .or_default()
                        .insert(cn.location.file_path.clone());
                }
            }
        }
    }

    // Find SCCs using iterative Tarjan's
    let files: Vec<String> = file_deps.keys().cloned().collect();
    let sccs = tarjan_scc(&files, &file_deps);

    let cycles: Vec<Value> = sccs.iter()
        .filter(|scc| scc.len() > 1)
        .map(|scc| {
            let files: Vec<String> = scc.iter().map(|f| strip_base(f, codebase)).collect();
            json!({"files": files, "size": scc.len()})
        })
        .collect();

    json!({"circular_dependencies": cycles, "total": cycles.len()})
}

/// Static test coverage map: which files have associated test files.
pub fn handle_test_coverage_map(graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let nodes = graph.find_nodes_by_name("", false);
    let mut source_files: HashSet<String> = HashSet::new();
    let mut test_files: HashSet<String> = HashSet::new();

    for node in &nodes {
        let fp = &node.location.file_path;
        let basename = Path::new(fp).file_name().unwrap_or_default().to_string_lossy();
        if basename.starts_with("test_") || basename.ends_with("_test.rs") || fp.contains("/tests/") || fp.contains("/test/") {
            test_files.insert(fp.clone());
        } else {
            source_files.insert(fp.clone());
        }
    }

    // Match test files to source files by naming convention
    let mut covered: Vec<String> = Vec::new();
    let mut uncovered: Vec<String> = Vec::new();

    for src in &source_files {
        let src_stem = Path::new(src).file_stem().unwrap_or_default().to_string_lossy();
        let has_test = test_files.iter().any(|t| {
            let t_stem = Path::new(t).file_stem().unwrap_or_default().to_string_lossy();
            t_stem == format!("test_{}", src_stem) || t_stem == format!("{}_test", src_stem)
        });
        if has_test {
            covered.push(strip_base(src, codebase));
        } else {
            uncovered.push(strip_base(src, codebase));
        }
    }

    let coverage_pct = if source_files.is_empty() { 0.0 } else { covered.len() as f64 / source_files.len() as f64 * 100.0 };

    json!({
        "coverage_percent": (coverage_pct * 10.0).round() / 10.0,
        "covered_files": covered.len(),
        "uncovered_files": uncovered.len(),
        "test_files": test_files.len(),
        "uncovered": uncovered.into_iter().take(20).collect::<Vec<_>>(),
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn strip_base(file: &str, codebase: Option<&str>) -> String {
    codebase
        .and_then(|b| Path::new(file).strip_prefix(b).ok())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| file.to_string())
}

/// Simple iterative Tarjan's SCC.
fn tarjan_scc(nodes: &[String], edges: &HashMap<String, HashSet<String>>) -> Vec<Vec<String>> {
    let mut index_counter = 0u32;
    let mut stack: Vec<String> = Vec::new();
    let mut on_stack: HashSet<String> = HashSet::new();
    let mut indices: HashMap<String, u32> = HashMap::new();
    let mut lowlinks: HashMap<String, u32> = HashMap::new();
    let mut result: Vec<Vec<String>> = Vec::new();

    fn strongconnect(
        v: &str, nodes_edges: &HashMap<String, HashSet<String>>,
        index_counter: &mut u32, stack: &mut Vec<String>, on_stack: &mut HashSet<String>,
        indices: &mut HashMap<String, u32>, lowlinks: &mut HashMap<String, u32>,
        result: &mut Vec<Vec<String>>,
    ) {
        indices.insert(v.to_string(), *index_counter);
        lowlinks.insert(v.to_string(), *index_counter);
        *index_counter += 1;
        stack.push(v.to_string());
        on_stack.insert(v.to_string());

        if let Some(neighbors) = nodes_edges.get(v) {
            for w in neighbors {
                if !indices.contains_key(w.as_str()) {
                    strongconnect(w, nodes_edges, index_counter, stack, on_stack, indices, lowlinks, result);
                    let wl = *lowlinks.get(w.as_str()).unwrap_or(&0);
                    let vl = lowlinks.get_mut(v).unwrap();
                    if wl < *vl { *vl = wl; }
                } else if on_stack.contains(w.as_str()) {
                    let wi = *indices.get(w.as_str()).unwrap_or(&0);
                    let vl = lowlinks.get_mut(v).unwrap();
                    if wi < *vl { *vl = wi; }
                }
            }
        }

        if lowlinks.get(v) == indices.get(v) {
            let mut scc = Vec::new();
            loop {
                let w = stack.pop().unwrap();
                on_stack.remove(&w);
                scc.push(w.clone());
                if w == v { break; }
            }
            result.push(scc);
        }
    }

    for node in nodes {
        if !indices.contains_key(node.as_str()) {
            strongconnect(node, edges, &mut index_counter, &mut stack, &mut on_stack, &mut indices, &mut lowlinks, &mut result);
        }
    }
    result
}
