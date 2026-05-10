//! Code tool: AST operations dispatch.

use std::path::Path;

use contextro_engines::graph::CodeGraph;
use contextro_parsing::TreeSitterParser;
use contextro_core::traits::Parser;
use serde_json::{json, Value};

pub fn handle_code(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let operation = args.get("operation").and_then(|v| v.as_str()).unwrap_or("");
    match operation {
        "get_document_symbols" => get_document_symbols(args),
        "search_symbols" => search_symbols(args, graph, codebase),
        "lookup_symbols" => lookup_symbols(args, graph, codebase),
        "pattern_search" => pattern_search(args, codebase),
        "search_codebase_map" => search_codebase_map(args, codebase),
        _ => json!({"error": format!("Unknown code operation: {}", operation)}),
    }
}

fn get_document_symbols(args: &Value) -> Value {
    let file_path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
    if file_path.is_empty() {
        return json!({"error": "Missing required parameter: file_path"});
    }
    if !Path::new(file_path).exists() {
        return json!({"error": format!("File not found: {}", file_path)});
    }

    let parser = TreeSitterParser::new();
    match parser.parse_file(file_path) {
        Ok(parsed) => {
            let symbols: Vec<Value> = parsed.symbols.iter().map(|s| {
                json!({
                    "name": s.name,
                    "type": s.symbol_type.to_string(),
                    "line": s.line_start,
                    "end_line": s.line_end,
                    "signature": s.signature,
                })
            }).collect();
            json!({"file": file_path, "symbols": symbols, "total": symbols.len()})
        }
        Err(e) => json!({"error": format!("Parse failed: {}", e)}),
    }
}

fn search_symbols(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let name = args.get("symbol_name").and_then(|v| v.as_str()).unwrap_or("");
    if name.is_empty() {
        return json!({"error": "Missing required parameter: symbol_name"});
    }

    let matches = graph.find_nodes_by_name(name, false);
    let symbols: Vec<Value> = matches.iter().take(20).map(|n| {
        let fp = codebase.map(|b| Path::new(&n.location.file_path).strip_prefix(b).map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| n.location.file_path.clone())).unwrap_or_else(|| n.location.file_path.clone());
        json!({"name": n.name, "type": n.node_type.to_string(), "file": fp, "line": n.location.start_line})
    }).collect();

    json!({"query": name, "symbols": symbols, "total": symbols.len()})
}

fn lookup_symbols(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let symbols_str = args.get("symbols").and_then(|v| v.as_str()).unwrap_or("");
    if symbols_str.is_empty() {
        return json!({"error": "Missing required parameter: symbols"});
    }

    let names: Vec<&str> = symbols_str.split(',').map(|s| s.trim()).collect();
    let mut results = Vec::new();

    for name in names {
        let matches = graph.find_nodes_by_name(name, true);
        for node in matches.iter().take(3) {
            let fp = codebase.map(|b| Path::new(&node.location.file_path).strip_prefix(b).map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| node.location.file_path.clone())).unwrap_or_else(|| node.location.file_path.clone());
            results.push(json!({
                "name": node.name,
                "type": node.node_type.to_string(),
                "file": fp,
                "line": node.location.start_line,
            }));
        }
    }

    json!({"symbols": results, "total": results.len()})
}

fn pattern_search(args: &Value, codebase: Option<&str>) -> Value {
    let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
    if pattern.is_empty() {
        return json!({"error": "Missing required parameter: pattern"});
    }
    // TODO: ast-grep pattern search
    json!({"pattern": pattern, "matches": [], "total": 0, "note": "ast-grep integration pending"})
}

fn search_codebase_map(args: &Value, codebase: Option<&str>) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let target = if Path::new(path).is_absolute() {
        path.to_string()
    } else {
        codebase.map(|b| format!("{}/{}", b, path)).unwrap_or_else(|| path.to_string())
    };

    if !Path::new(&target).is_dir() {
        return json!({"error": format!("Not a directory: {}", target)});
    }

    let entries: Vec<Value> = std::fs::read_dir(&target)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .take(50)
        .map(|e| {
            let is_dir = e.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            json!({"name": e.file_name().to_string_lossy().to_string(), "is_dir": is_dir})
        })
        .collect();

    json!({"path": path, "entries": entries, "total": entries.len()})
}
