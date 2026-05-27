use super::*;

pub(crate) fn search_symbols(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    // Accept `symbol_name` (current) or `query` (v0.4.0 alias)
    let name = args
        .get("symbol_name")
        .or_else(|| args.get("name"))
        .or_else(|| args.get("query"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if name.is_empty() {
        return json!({"error": "Missing required parameter: symbol_name", "hint": "Use symbol_name (preferred) or query for the search_symbols operation."});
    }

    let matches = graph.find_nodes_by_name(name, false);
    let symbols: Vec<Value> = matches.iter().take(20).map(|n| {
        let fp = strip_base(&n.location.file_path, codebase);
        json!({"name": n.name, "type": n.node_type.to_string(), "file": fp, "line": n.location.start_line})
    }).collect();

    json!({"query": name, "symbols": symbols, "total": symbols.len()})
}

pub(crate) fn lookup_symbols(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    // Accept symbols as a JSON array ["A","B"] or comma-separated string "A,B"
    let names: Vec<String> = match args.get("symbols") {
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        Some(Value::String(s)) if !s.is_empty() => s
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => {
            return json!({"error": "Missing required parameter: symbols (comma-separated string or JSON array)"})
        }
    };
    if names.is_empty() {
        return json!({"error": "Parameter 'symbols' must contain at least one symbol name."});
    }

    let include_source = args
        .get("include_source")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut results = Vec::new();

    for name in &names {
        let matches = graph.find_nodes_by_name(name.as_str(), true);
        let include_type = include_source || matches.len() > 1;
        for node in matches.iter().take(3) {
            let fp = strip_base(&node.location.file_path, codebase);
            let mut entry = json!({
                "name": node.name,
                "file": fp,
                "line": node.location.start_line,
            });
            if include_type {
                entry["type"] = json!(node.node_type.to_string());
            }
            if include_source {
                // Read source lines from file
                if let Ok(content) = std::fs::read_to_string(&node.location.file_path) {
                    let lines: Vec<&str> = content.lines().collect();
                    let start = (node.location.start_line as usize).saturating_sub(1);
                    let end = (node.location.end_line as usize).min(lines.len());
                    let source = lines[start..end].join("\n");
                    entry["source"] = json!(source);
                }
            }
            results.push(entry);
        }
    }

    json!({"symbols": results, "total": results.len()})
}

pub(crate) fn list_symbols(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return json!({"error": "Missing required parameter: path"});
    }
    let abs_path = match resolve_existing_path(path, codebase) {
        Ok(path) => path,
        Err(error) => return error,
    };
    let is_dir = abs_path.is_dir();

    let all_nodes = graph.find_nodes_by_name("", false);
    let symbols: Vec<Value> = all_nodes
        .iter()
        .filter(|n| path_matches(&n.location.file_path, &abs_path, is_dir))
        .map(|n| {
            let fp = strip_base(&n.location.file_path, codebase);
            let (callers, callees) = graph.get_node_degree(&n.id);
            json!({
                "name": n.name,
                "type": n.node_type.to_string(),
                "file": fp,
                "line": n.location.start_line,
                "callers": callers,
                "callees": callees,
            })
        })
        .collect();

    json!({"path": strip_base(&abs_path.to_string_lossy(), codebase), "symbols": symbols, "total": symbols.len()})
}
