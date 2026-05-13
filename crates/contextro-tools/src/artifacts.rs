//! Artifact tools: audit, docs_bundle, sidecar_export, skill_prompt, introspect.

use std::collections::HashMap;
use std::path::Path;

use contextro_engines::graph::CodeGraph;
use serde_json::{json, Value};

/// Generate an audit report with recommendations.
pub fn handle_audit(graph: &CodeGraph, _codebase: Option<&str>) -> Value {
    let nodes = graph.find_nodes_by_name("", false);
    let mut recommendations: Vec<Value> = Vec::new();

    // Check for high-complexity symbols
    let mut high_conn = 0;
    for node in &nodes {
        let (in_d, out_d) = graph.get_node_degree(&node.id);
        if in_d + out_d > 10 {
            high_conn += 1;
        }
    }
    if high_conn > 0 {
        recommendations.push(json!({
            "severity": "medium",
            "category": "complexity",
            "message": format!("{} symbols have >10 connections — consider refactoring", high_conn),
        }));
    }

    // Check file concentration
    let mut file_counts: HashMap<String, usize> = HashMap::new();
    for node in &nodes {
        *file_counts
            .entry(node.location.file_path.clone())
            .or_default() += 1;
    }
    let large_files: Vec<_> = file_counts.iter().filter(|(_, c)| **c > 30).collect();
    if !large_files.is_empty() {
        recommendations.push(json!({
            "severity": "low",
            "category": "structure",
            "message": format!("{} files have >30 symbols — consider splitting", large_files.len()),
        }));
    }

    let quality_score = if recommendations.is_empty() {
        95
    } else {
        85 - recommendations.len() * 5
    };

    json!({
        "status": "complete",
        "quality_score": quality_score,
        "total_symbols": nodes.len(),
        "recommendations": recommendations,
    })
}

/// Generate a docs bundle.
pub fn handle_docs_bundle(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let output_dir = args
        .get("output_dir")
        .and_then(|v| v.as_str())
        .unwrap_or(".contextro-docs");
    let base = codebase.unwrap_or(".");
    let target = if Path::new(output_dir).is_absolute() {
        output_dir.to_string()
    } else {
        format!("{}/{}", base, output_dir)
    };

    std::fs::create_dir_all(&target).ok();

    // Generate architecture.md
    let nodes = graph.find_nodes_by_name("", false);
    let mut arch = String::from("# Architecture\n\n## Hub Symbols\n\n");
    let mut scored: Vec<_> = nodes
        .iter()
        .map(|n| {
            let (i, o) = graph.get_node_degree(&n.id);
            (n.name.clone(), n.location.file_path.clone(), i + o)
        })
        .collect();
    scored.sort_by_key(|b| std::cmp::Reverse(b.2));
    for (name, file, degree) in scored.iter().take(10) {
        arch.push_str(&format!(
            "- **{}** ({}) — {} connections\n",
            name, file, degree
        ));
    }
    std::fs::write(format!("{}/architecture.md", target), &arch).ok();

    // Generate overview.md
    let overview = format!(
        "# Overview\n\n- Total symbols: {}\n- Total relationships: {}\n",
        graph.node_count(),
        graph.relationship_count()
    );
    std::fs::write(format!("{}/overview.md", target), &overview).ok();

    json!({"status": "generated", "output_dir": target, "files": ["architecture.md", "overview.md"]})
}

/// Generate .graph.* sidecar files.
pub fn handle_sidecar_export(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let base = codebase.unwrap_or(".");
    let output_dir = args
        .get("output_dir")
        .and_then(|v| v.as_str())
        .unwrap_or(".contextro-sidecars");
    let target = if path == "." || path.is_empty() {
        base.to_string()
    } else if Path::new(path).is_absolute() {
        path.to_string()
    } else {
        format!("{}/{}", base, path)
    };

    // Resolve output directory
    let out_base = if Path::new(output_dir).is_absolute() {
        output_dir.to_string()
    } else {
        format!("{}/{}", base, output_dir)
    };
    std::fs::create_dir_all(&out_base).ok();

    let nodes = graph.find_nodes_by_name("", false);
    let mut files_written = 0;

    // Group symbols by file
    let mut by_file: HashMap<String, Vec<&_>> = HashMap::new();
    for node in &nodes {
        if node.location.file_path.starts_with(&target) {
            by_file
                .entry(node.location.file_path.clone())
                .or_default()
                .push(node);
        }
    }

    for (file_path, syms) in &by_file {
        // Write to output directory with relative path structure
        let rel = Path::new(file_path)
            .strip_prefix(base)
            .unwrap_or(Path::new(file_path));
        let sidecar_name = format!("{}.graph.md", rel.to_string_lossy());
        let sidecar_path = format!("{}/{}", out_base, sidecar_name);

        // Create parent directories
        if let Some(parent) = Path::new(&sidecar_path).parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let mut content = format!(
            "# {}\n\n## Symbols\n\n",
            Path::new(file_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        );
        for sym in syms {
            let (in_d, out_d) = graph.get_node_degree(&sym.id);
            content.push_str(&format!(
                "- `{}` ({}) L{} — {} callers, {} callees\n",
                sym.name, sym.node_type, sym.location.start_line, in_d, out_d
            ));
        }
        if std::fs::write(&sidecar_path, &content).is_ok() {
            files_written += 1;
        }
    }

    json!({"status": "exported", "sidecars": files_written, "path": path, "output_dir": out_base})
}

/// Print the agent bootstrap block.
pub fn handle_skill_prompt() -> Value {
    json!({
        "bootstrap": "# Contextro\n\nUse `index(path)` to index a codebase, then:\n- `search(query)` — semantic + keyword hybrid search\n- `find_symbol(symbol_name)` — locate definitions\n- `find_callers(symbol_name)` / `find_callees(symbol_name)` — call graph\n- `explain(symbol_name)` — symbol summary and context\n- `impact(symbol_name)` — blast radius\n- `remember(content)` / `recall(query)` — persistent memory\n- `code(operation, ...)` — AST operations\n",
    })
}

/// Look up Contextro's own tool docs.
pub fn handle_introspect(args: &Value) -> Value {
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");

    let tools: Vec<(&str, &str)> = vec![
        ("index", "Index a codebase. Args: path (required)"),
        ("search", "Semantic + keyword + graph hybrid search. Args: query, limit, mode, language"),
        ("find_symbol", "Find a symbol definition. Args: symbol_name (preferred), name/symbol aliases, exact"),
        ("find_callers", "Who calls this function? Args: symbol_name"),
        ("find_callees", "What does this function call? Args: symbol_name"),
        ("explain", "Natural-language symbol summary plus callers/callees/docstring. Args: symbol_name"),
        ("impact", "What breaks if I change this? Args: symbol_name, max_depth"),
        ("code", "AST operations. Args: operation (get_document_symbols|search_symbols|lookup_symbols|pattern_search|search_codebase_map); use path as the preferred file/directory parameter"),
        ("overview", "Project overview: totals, languages, symbol types, top files/directories. No args."),
        ("architecture", "Layers, entry points, hub symbols. No args."),
        ("analyze", "Code smells and complexity. Args: path (optional)"),
        ("focus", "Low-token context slice. Args: path"),
        ("dead_code", "Entry-point reachability analysis. No args."),
        ("circular_dependencies", "SCC-based circular deps. No args."),
        ("test_coverage_map", "Static heuristic test coverage map (not runtime coverage). No args."),
        ("remember", "Store a note/decision. Args: content, memory_type, tags, ttl"),
        ("recall", "Search memories. Args: query, limit, memory_type, tags"),
        ("forget", "Delete memories. Args: memory_id, tags, memory_type"),
        ("knowledge", "Index/search docs. Args: command (show|add|search|remove|update), name, query, value"),
        ("compact", "Archive session content. Args: content"),
        ("session_snapshot", "Show recent tool calls with arguments. No args."),
        ("restore", "Project re-entry summary. No args."),
        ("retrieve", "Fetch sandboxed output. Args: ref_id"),
        ("commit_search", "Semantic search over git history. Args: query, limit"),
        ("commit_history", "Browse recent commits. Args: limit"),
        ("repo_add", "Register another repo. Args: path"),
        ("repo_remove", "Unregister a repo. Args: path"),
        ("repo_status", "View all repos. No args."),
        ("audit", "Packaged audit report. No args."),
        ("docs_bundle", "Generate docs bundle. Args: output_dir"),
        ("sidecar_export", "Generate .graph.* sidecars. Args: path"),
        ("skill_prompt", "Print agent bootstrap block. No args."),
        ("introspect", "Look up Contextro docs. Args: query"),
        ("status", "Server status. No args."),
        ("health", "Health check. No args."),
    ];

    if query.is_empty() {
        let all: Vec<Value> = tools
            .iter()
            .map(|(n, d)| json!({"tool": n, "description": d}))
            .collect();
        return json!({"tools": all, "total": all.len()});
    }

    // Match tools where ANY query word appears in name or description.
    // Rank by number of matching words (more matches = more relevant).
    let words: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(|w| w.to_string())
        .collect();

    let mut scored: Vec<(usize, &str, &str)> = tools
        .iter()
        .filter_map(|(n, d)| {
            let haystack = format!("{} {}", n.to_lowercase(), d.to_lowercase());
            let hits = words
                .iter()
                .filter(|w| haystack.contains(w.as_str()))
                .count();
            if hits > 0 {
                Some((hits, *n, *d))
            } else {
                None
            }
        })
        .collect();
    scored.sort_by_key(|(hits, _, _)| std::cmp::Reverse(*hits));

    let matching: Vec<Value> = scored
        .iter()
        .map(|(_, n, d)| json!({"tool": n, "description": d}))
        .collect();

    json!({"query": query, "matching_tools": matching, "total": matching.len()})
}
