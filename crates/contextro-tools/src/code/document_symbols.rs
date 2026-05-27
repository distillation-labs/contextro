use super::*;

pub(crate) fn get_document_symbols(
    args: &Value,
    graph: Option<&CodeGraph>,
    codebase: Option<&str>,
) -> Value {
    let file_path = match get_document_path_arg(args) {
        Some(path) => path,
        None => return json!({"error": "Missing required parameter: path"}),
    };
    let include_signature = args
        .get("include_signature")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let symbol_limit = document_symbol_limit(args, include_signature);
    let abs_path = match resolve_existing_path(file_path, codebase) {
        Ok(path) => path,
        Err(error) => return error,
    };
    if !abs_path.is_file() {
        return json!({"error": format!("Path is not a file: {}", file_path)});
    }

    if !include_signature {
        if let Some(graph) = graph {
            for candidate in graph_document_path_candidates(&abs_path, file_path, codebase) {
                let indexed = graph.get_nodes_by_file(&candidate);
                if !indexed.is_empty() {
                    return render_indexed_document_symbols(
                        &abs_path,
                        &indexed,
                        symbol_limit,
                        codebase,
                    );
                }
            }
        }
    }

    let parser = TreeSitterParser::new();
    match parser.parse_file(abs_path.to_string_lossy().as_ref()) {
        Ok(parsed) => render_parsed_document_symbols(
            &abs_path,
            &parsed.symbols,
            include_signature,
            symbol_limit,
            codebase,
        ),
        Err(e) => json!({"error": format!("Parse failed: {}", e)}),
    }
}

pub(crate) fn document_symbol_limit(args: &Value, include_signature: bool) -> Option<usize> {
    if let Some(limit) = args.get("limit").and_then(|value| value.as_u64()) {
        return (limit > 0).then_some(limit as usize);
    }

    (!include_signature).then_some(DEFAULT_DOCUMENT_SYMBOL_LIMIT)
}

pub(crate) fn render_parsed_document_symbols(
    abs_path: &Path,
    symbols: &[contextro_core::models::Symbol],
    include_signature: bool,
    symbol_limit: Option<usize>,
    codebase: Option<&str>,
) -> Value {
    let total_symbols = symbols.len();
    let truncated = symbol_limit
        .map(|limit| total_symbols > limit)
        .unwrap_or(false);
    let mut columns = vec![json!("name"), json!("type"), json!("line")];
    let has_multiline = symbols
        .iter()
        .any(|symbol| symbol.line_end > symbol.line_start + 1);
    if has_multiline {
        columns.push(json!("end_line"));
    }
    if include_signature {
        columns.push(json!("signature"));
    }

    let mut rows: Vec<Value> = symbols
        .iter()
        .map(|s| {
            let mut row = vec![
                json!(s.name),
                json!(s.symbol_type.to_string()),
                json!(s.line_start),
            ];
            if has_multiline {
                if s.line_end > s.line_start + 1 {
                    row.push(json!(s.line_end));
                } else {
                    row.push(Value::Null);
                }
            }
            if include_signature {
                // Truncate long signatures to bound payload size when callers opt in.
                let sig = if s.signature.chars().count() > 60 {
                    truncate_chars(&s.signature, 57)
                } else {
                    s.signature.clone()
                };
                row.push(json!(sig));
            }
            Value::Array(row)
        })
        .collect();

    if let Some(limit) = symbol_limit {
        rows.truncate(limit);
    }

    let mut response = json!({
        "file": strip_base(&abs_path.to_string_lossy(), codebase),
        "columns": columns,
        "symbols": rows,
        "total": total_symbols
    });
    if truncated {
        response["truncated"] = json!(true);
    }
    response
}

pub(crate) fn render_indexed_document_symbols(
    abs_path: &Path,
    indexed: &[UniversalNode],
    symbol_limit: Option<usize>,
    codebase: Option<&str>,
) -> Value {
    let mut sorted = indexed.to_vec();
    sorted.sort_by(|a, b| {
        a.location
            .start_line
            .cmp(&b.location.start_line)
            .then_with(|| a.location.end_line.cmp(&b.location.end_line))
            .then_with(|| a.name.cmp(&b.name))
    });

    let total_symbols = sorted.len();
    let truncated = symbol_limit
        .map(|limit| total_symbols > limit)
        .unwrap_or(false);
    let has_multiline = sorted
        .iter()
        .any(|symbol| symbol.location.end_line > symbol.location.start_line + 1);
    let mut columns = vec![json!("name"), json!("type"), json!("line")];
    if has_multiline {
        columns.push(json!("end_line"));
    }

    let mut rows: Vec<Value> = sorted
        .iter()
        .map(|symbol| {
            let mut row = vec![
                json!(symbol.name),
                json!(document_symbol_type(symbol)),
                json!(symbol.location.start_line),
            ];
            if has_multiline {
                if symbol.location.end_line > symbol.location.start_line + 1 {
                    row.push(json!(symbol.location.end_line));
                } else {
                    row.push(Value::Null);
                }
            }
            Value::Array(row)
        })
        .collect();

    if let Some(limit) = symbol_limit {
        rows.truncate(limit);
    }

    let mut response = json!({
        "file": strip_base(&abs_path.to_string_lossy(), codebase),
        "columns": columns,
        "symbols": rows,
        "total": total_symbols
    });
    if truncated {
        response["truncated"] = json!(true);
    }
    response
}

pub(crate) fn document_symbol_type(node: &UniversalNode) -> &'static str {
    match node.node_type {
        contextro_core::graph::NodeType::Class => "class",
        contextro_core::graph::NodeType::Variable => "variable",
        contextro_core::graph::NodeType::Function if node.parent.is_some() => "method",
        contextro_core::graph::NodeType::Function => "function",
        _ => "function",
    }
}

pub(crate) fn graph_document_path_candidates(
    abs_path: &Path,
    requested_path: &str,
    codebase: Option<&str>,
) -> Vec<String> {
    let mut candidates = Vec::with_capacity(4);

    let abs = abs_path.to_string_lossy().to_string();
    candidates.push(abs.clone());

    if let Some(base) = codebase {
        let canonical_base = std::fs::canonicalize(base).unwrap_or_else(|_| PathBuf::from(base));
        if let Ok(relative) = abs_path.strip_prefix(&canonical_base) {
            let relative = relative.to_string_lossy().to_string();
            if !relative.is_empty() && !candidates.contains(&relative) {
                candidates.push(relative.clone());
            }

            let dotted = format!("./{relative}");
            if !relative.is_empty() && !candidates.contains(&dotted) {
                candidates.push(dotted);
            }
        }
    }

    if !Path::new(requested_path).is_absolute() {
        let requested = requested_path.to_string();
        if !candidates.contains(&requested) {
            candidates.push(requested);
        }
    }

    candidates
}
