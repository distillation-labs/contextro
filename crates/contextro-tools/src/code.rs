//! Code tool: AST operations dispatch.

use std::path::Path;

use contextro_core::traits::Parser;
use contextro_engines::graph::CodeGraph;
use contextro_parsing::TreeSitterParser;
use serde_json::{json, Value};

pub fn handle_code(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    // Accept both `operation` (current) and `action` (v0.4.0 name) for backward compat
    let operation = args
        .get("operation")
        .or_else(|| args.get("action"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    match operation {
        "get_document_symbols" => get_document_symbols(args),
        // v0.4.0 name alias
        "list_symbols" => {
            // If `file_path` or `path` point to a file, use get_document_symbols;
            // otherwise fall through to the directory-based list
            let has_file = args.get("file_path").and_then(|v| v.as_str()).is_some();
            if has_file {
                get_document_symbols(args)
            } else {
                list_symbols(args, graph, codebase)
            }
        }
        "search_symbols" => search_symbols(args, graph, codebase),
        "lookup_symbols" => lookup_symbols(args, graph, codebase),
        "pattern_search" => pattern_search(args, codebase),
        "pattern_rewrite" => pattern_rewrite(args, codebase),
        "edit_plan" => edit_plan(args, graph, codebase),
        "search_codebase_map" => search_codebase_map(args, graph, codebase),
        _ => {
            json!({"error": format!("Unknown code operation: '{}'. Valid operations: get_document_symbols, search_symbols, lookup_symbols, list_symbols, pattern_search, pattern_rewrite, edit_plan, search_codebase_map", operation)})
        }
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
            let symbols: Vec<Value> = parsed
                .symbols
                .iter()
                .map(|s| {
                    json!({
                        "name": s.name,
                        "type": s.symbol_type.to_string(),
                        "line": s.line_start,
                        "end_line": s.line_end,
                        "signature": s.signature,
                    })
                })
                .collect();
            json!({"file": file_path, "symbols": symbols, "total": symbols.len()})
        }
        Err(e) => json!({"error": format!("Parse failed: {}", e)}),
    }
}

fn search_symbols(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    // Accept `symbol_name` (current) or `query` (v0.4.0 alias)
    let name = args
        .get("symbol_name")
        .or_else(|| args.get("query"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if name.is_empty() {
        return json!({"error": "Missing required parameter: symbol_name"});
    }

    let matches = graph.find_nodes_by_name(name, false);
    let symbols: Vec<Value> = matches.iter().take(20).map(|n| {
        let fp = strip_base(&n.location.file_path, codebase);
        json!({"name": n.name, "type": n.node_type.to_string(), "file": fp, "line": n.location.start_line})
    }).collect();

    json!({"query": name, "symbols": symbols, "total": symbols.len()})
}

fn lookup_symbols(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
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
        return json!({"error": "Missing required parameter: symbols"});
    }

    let include_source = args
        .get("include_source")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut results = Vec::new();

    for name in &names {
        let matches = graph.find_nodes_by_name(name.as_str(), true);
        for node in matches.iter().take(3) {
            let fp = strip_base(&node.location.file_path, codebase);
            let mut entry = json!({
                "name": node.name,
                "type": node.node_type.to_string(),
                "file": fp,
                "line": node.location.start_line,
            });
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

fn list_symbols(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return json!({"error": "Missing required parameter: path"});
    }
    let base = codebase.unwrap_or(".");
    let abs_path = if Path::new(path).is_absolute() {
        path.to_string()
    } else {
        format!("{}/{}", base, path)
    };

    let all_nodes = graph.find_nodes_by_name("", false);
    let symbols: Vec<Value> = all_nodes
        .iter()
        .filter(|n| {
            n.location.file_path == abs_path
                || n.location.file_path.starts_with(&format!("{}/", abs_path))
        })
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

    json!({"path": path, "symbols": symbols, "total": symbols.len()})
}

/// Pattern search using grep-style matching with structural awareness.
fn pattern_search(args: &Value, codebase: Option<&str>) -> Value {
    let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
    if pattern.is_empty() {
        return json!({"error": "Missing required parameter: pattern"});
    }
    let language = args.get("language").and_then(|v| v.as_str());
    let file_path = args.get("file_path").and_then(|v| v.as_str());
    let search_path = args.get("path").and_then(|v| v.as_str());

    let base = codebase.unwrap_or(".");
    let target = file_path
        .or(search_path)
        .map(|p| {
            if Path::new(p).is_absolute() {
                p.to_string()
            } else {
                format!("{}/{}", base, p)
            }
        })
        .unwrap_or_else(|| base.to_string());

    // If the pattern contains ast-grep metavariables ($NAME, $$$), convert them.
    // Otherwise, treat the pattern as a regex first; fall back to literal string
    // matching if the regex is invalid. This lets callers use "impl.*Engine" style
    // patterns without needing to know about ast-grep syntax.
    let re = if pattern.contains('$') {
        let regex_pattern = pattern_to_regex(pattern);
        regex_lite::Regex::new(&regex_pattern)
            .unwrap_or_else(|_| regex_lite::Regex::new(&regex_lite::escape(pattern)).unwrap())
    } else {
        regex_lite::Regex::new(pattern)
            .unwrap_or_else(|_| regex_lite::Regex::new(&regex_lite::escape(pattern)).unwrap())
    };

    let mut matches: Vec<Value> = Vec::new();
    let files = collect_files(&target, language);

    for file in &files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for (line_num, line) in content.lines().enumerate() {
            if re.is_match(line) {
                matches.push(json!({
                    "file": strip_base(file, codebase),
                    "line": line_num + 1,
                    "code": line.trim(),
                }));
                if matches.len() >= 50 {
                    return json!({"pattern": pattern, "matches": matches, "total": matches.len(), "truncated": true});
                }
            }
        }
    }

    json!({"pattern": pattern, "matches": matches, "total": matches.len()})
}

/// Pattern rewrite: find and replace using structural patterns.
fn pattern_rewrite(args: &Value, codebase: Option<&str>) -> Value {
    let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
    let replacement = args
        .get("replacement")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let dry_run = args
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    if pattern.is_empty() || replacement.is_empty() {
        return json!({"error": "Missing required parameters: pattern and replacement"});
    }

    let file_path = args.get("file_path").and_then(|v| v.as_str());
    let search_path = args.get("path").and_then(|v| v.as_str());
    let language = args.get("language").and_then(|v| v.as_str());
    let base = codebase.unwrap_or(".");

    let target = file_path
        .or(search_path)
        .map(|p| {
            if Path::new(p).is_absolute() {
                p.to_string()
            } else {
                format!("{}/{}", base, p)
            }
        })
        .unwrap_or_else(|| base.to_string());

    let regex_pattern = pattern_to_regex(pattern);
    let re = match regex_lite::Regex::new(&regex_pattern) {
        Ok(r) => r,
        Err(_) => return json!({"error": "Invalid pattern"}),
    };

    let files = collect_files(&target, language);
    let mut changes: Vec<Value> = Vec::new();
    let mut total_replacements = 0;

    for file in &files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let new_content = re.replace_all(&content, replacement);
        if new_content != content {
            let count = content
                .lines()
                .zip(new_content.lines())
                .filter(|(a, b)| a != b)
                .count();
            total_replacements += count;

            // Generate diff
            let diff_lines: Vec<String> = content
                .lines()
                .zip(new_content.lines())
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .take(5)
                .map(|(i, (old, new))| {
                    format!("L{}: -{}\nL{}: +{}", i + 1, old.trim(), i + 1, new.trim())
                })
                .collect();

            changes.push(json!({
                "file": strip_base(file, codebase),
                "replacements": count,
                "diff_preview": diff_lines.join("\n"),
            }));

            if !dry_run {
                std::fs::write(file, new_content.as_ref()).ok();
            }
        }
    }

    json!({
        "pattern": pattern,
        "replacement": replacement,
        "dry_run": dry_run,
        "changes": changes,
        "total_files": changes.len(),
        "total_replacements": total_replacements,
    })
}

/// Edit plan: analyze scope, impact, and recommend approach.
fn edit_plan(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let goal = args.get("goal").and_then(|v| v.as_str()).unwrap_or("");
    if goal.is_empty() {
        return json!({"error": "Missing required parameter: goal"});
    }
    let file_path = args.get("file_path").and_then(|v| v.as_str());
    let pattern = args.get("pattern").and_then(|v| v.as_str());
    let symbol_name = args.get("symbol_name").and_then(|v| v.as_str());

    let mut target_files: Vec<String> = Vec::new();
    let mut affected_symbols: Vec<Value> = Vec::new();
    let mut risks: Vec<String> = Vec::new();

    // Find target symbols
    if let Some(sym) = symbol_name {
        let matches = graph.find_nodes_by_name(sym, false);
        for node in matches.iter().take(5) {
            target_files.push(strip_base(&node.location.file_path, codebase));
            let (in_d, _) = graph.get_node_degree(&node.id);
            affected_symbols.push(json!({"name": node.name, "file": strip_base(&node.location.file_path, codebase), "callers": in_d}));
            if in_d > 5 {
                risks.push(format!(
                    "{} has {} callers — high blast radius",
                    node.name, in_d
                ));
            }
        }
    }

    if let Some(fp) = file_path {
        if !target_files.contains(&fp.to_string()) {
            target_files.push(fp.to_string());
        }
    }

    // Find related tests
    let related_tests: Vec<String> = target_files
        .iter()
        .filter_map(|f| {
            let stem = Path::new(f).file_stem()?.to_string_lossy().to_string();
            let test_name = format!("test_{}", stem);
            if graph.find_nodes_by_name(&test_name, false).is_empty() {
                None
            } else {
                Some(test_name)
            }
        })
        .collect();

    let mut next_steps = vec!["Review the diff preview before applying".to_string()];
    if pattern.is_some() {
        next_steps.insert(0, "Run pattern_rewrite with dry_run=true first".to_string());
    }
    if !related_tests.is_empty() {
        next_steps.push("Run related tests after applying".to_string());
    }

    json!({
        "goal": goal,
        "target_files": target_files,
        "affected_symbols": affected_symbols,
        "related_tests": related_tests,
        "risks": risks,
        "confidence": if risks.is_empty() { "high" } else { "medium" },
        "next_steps": next_steps,
    })
}

/// Return a symbol-level map of the codebase grouped by file.
/// Accepts an optional `query` to filter by symbol name and an optional `path` prefix.
fn search_codebase_map(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    let path_filter = args.get("path").and_then(|v| v.as_str()).unwrap_or("");

    let all_nodes = graph.find_nodes_by_name("", false);

    let mut file_map: std::collections::BTreeMap<String, Vec<Value>> =
        std::collections::BTreeMap::new();
    for node in &all_nodes {
        let rel = strip_base(&node.location.file_path, codebase);
        // Filter by path prefix ("." means no filter)
        if !path_filter.is_empty() && path_filter != "." && !rel.starts_with(path_filter) {
            continue;
        }
        // Filter by query
        if !query.is_empty() && !node.name.to_lowercase().contains(&query) {
            continue;
        }
        let (callers, callees) = graph.get_node_degree(&node.id);
        file_map.entry(rel).or_default().push(json!({
            "name": node.name,
            "type": node.node_type.to_string(),
            "line": node.location.start_line,
            "callers": callers,
            "callees": callees,
        }));
    }

    let files: Vec<Value> = file_map
        .into_iter()
        .map(|(file, symbols)| {
            let total = symbols.len();
            json!({"file": file, "symbols": symbols, "total": total})
        })
        .collect();

    let total_symbols: usize = files
        .iter()
        .map(|f| f["total"].as_u64().unwrap_or(0) as usize)
        .sum();

    json!({
        "path": if path_filter.is_empty() { "." } else { path_filter },
        "query": if query.is_empty() { Value::Null } else { json!(query) },
        "files": files,
        "total_files": files.len(),
        "total_symbols": total_symbols,
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn strip_base(file: &str, codebase: Option<&str>) -> String {
    codebase
        .and_then(|b| Path::new(file).strip_prefix(b).ok())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| file.to_string())
}

/// Convert ast-grep-style pattern to regex.
/// $NAME -> captures a word, $$$ -> captures anything.
fn pattern_to_regex(pattern: &str) -> String {
    let escaped = regex_lite::escape(pattern);
    // Replace escaped $$$  with .+
    let result = escaped.replace("\\$\\$\\$", ".+");
    // Replace escaped $WORD with \\w+
    let re = regex_lite::Regex::new(r"\\\$[A-Z_]+").unwrap();
    re.replace_all(&result, r"\w+").to_string()
}

/// Collect files from a path, optionally filtered by language extension.
/// Uses the `ignore` crate so it respects .gitignore and handles unlimited depth.
fn collect_files(path: &str, language: Option<&str>) -> Vec<String> {
    let p = Path::new(path);
    if p.is_file() {
        return vec![path.to_string()];
    }

    let extensions: Option<Vec<&str>> = language.map(|lang| match lang {
        "python" => vec!["py"],
        "rust" => vec!["rs"],
        "javascript" | "js" => vec!["js", "jsx"],
        "typescript" | "ts" => vec!["ts", "tsx"],
        "go" => vec!["go"],
        "java" => vec!["java"],
        "c" => vec!["c", "h"],
        "cpp" | "c++" => vec!["cpp", "hpp", "cc", "cxx"],
        "ruby" => vec!["rb"],
        _ => vec![],
    });

    let mut files = Vec::new();
    for entry in ignore::Walk::new(p).flatten() {
        let ep = entry.path();
        if !ep.is_file() {
            continue;
        }
        if let Some(exts) = &extensions {
            if let Some(ext) = ep.extension().and_then(|e| e.to_str()) {
                if exts.contains(&ext) {
                    files.push(ep.to_string_lossy().to_string());
                }
            }
        } else {
            // No language filter — include all non-binary files
            if let Some(ext) = ep.extension().and_then(|e| e.to_str()) {
                if ![
                    "png", "jpg", "jpeg", "gif", "svg", "ico", "woff", "woff2", "ttf", "eot",
                    "pdf", "zip", "gz", "tar", "lock", "map", "min.js",
                ]
                .contains(&ext)
                {
                    files.push(ep.to_string_lossy().to_string());
                }
            }
        }
    }
    files
}
