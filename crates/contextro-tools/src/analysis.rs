//! Analysis tools: overview, architecture, analyze.

use std::collections::HashMap;
use std::path::Path;

use contextro_engines::graph::CodeGraph;
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
    // Find hub symbols (highest degree)
    let nodes = graph.find_nodes_by_name("", false); // get all
    let mut scored: Vec<(String, String, usize)> = nodes.iter().map(|n| {
        let (in_d, out_d) = graph.get_node_degree(&n.id);
        (n.name.clone(), n.location.file_path.clone(), in_d + out_d)
    }).collect();
    scored.sort_by(|a, b| b.2.cmp(&a.2));

    let hubs: Vec<Value> = scored.iter().take(10).map(|(name, file, degree)| {
        let fp = codebase.map(|b| Path::new(file).strip_prefix(b).map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| file.clone())).unwrap_or_else(|| file.clone());
        json!({"name": name, "file": fp, "degree": degree})
    }).collect();

    json!({
        "hub_symbols": hubs,
        "total_nodes": graph.node_count(),
        "total_edges": graph.relationship_count(),
    })
}

pub fn handle_analyze(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let _path = args.get("path").and_then(|v| v.as_str());

    // Basic complexity analysis from graph
    let nodes = graph.find_nodes_by_name("", false);
    let mut complex_fns: Vec<Value> = Vec::new();
    let mut file_sizes: HashMap<String, usize> = HashMap::new();

    for node in &nodes {
        *file_sizes.entry(node.location.file_path.clone()).or_default() += 1;
        let (in_d, out_d) = graph.get_node_degree(&node.id);
        if in_d + out_d > 5 {
            let fp = codebase.map(|b| Path::new(&node.location.file_path).strip_prefix(b).map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| node.location.file_path.clone())).unwrap_or_else(|| node.location.file_path.clone());
            complex_fns.push(json!({"name": node.name, "file": fp, "connections": in_d + out_d}));
        }
    }
    complex_fns.sort_by(|a, b| b["connections"].as_u64().cmp(&a["connections"].as_u64()));
    complex_fns.truncate(10);

    let mut large_files: Vec<Value> = file_sizes.iter()
        .filter(|(_, count)| **count > 10)
        .map(|(file, count)| {
            let fp = codebase.map(|b| Path::new(file).strip_prefix(b).map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| file.clone())).unwrap_or_else(|| file.clone());
            json!({"file": fp, "symbols": count})
        })
        .collect();
    large_files.sort_by(|a, b| b["symbols"].as_u64().cmp(&a["symbols"].as_u64()));

    json!({
        "high_connectivity_symbols": complex_fns,
        "large_files": large_files,
        "total_symbols": nodes.len(),
    })
}
