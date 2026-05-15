//! Code tool: AST operations dispatch.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use crate::analysis::is_test_file;
use contextro_core::graph::UniversalNode;
use contextro_core::traits::Parser;
use contextro_engines::graph::CodeGraph;
use contextro_parsing::TreeSitterParser;
use serde_json::{json, Value};

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

pub fn handle_code(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    // Accept both `operation` (current) and `action` (v0.4.0 name) for backward compat
    let operation = args
        .get("operation")
        .or_else(|| args.get("action"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    match operation {
        "get_document_symbols" => get_document_symbols(args, codebase),
        // v0.4.0 name alias
        "list_symbols" => {
            // If `file_path` or `path` point to a file, use get_document_symbols;
            // otherwise fall through to the directory-based list
            let has_file = get_document_path_arg(args)
                .and_then(|path| resolve_existing_path(path, codebase).ok())
                .map(|path| path.is_file())
                .unwrap_or(false);
            if has_file {
                get_document_symbols(args, codebase)
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

fn get_document_symbols(args: &Value, codebase: Option<&str>) -> Value {
    let file_path = match get_document_path_arg(args) {
        Some(path) => path,
        None => return json!({"error": "Missing required parameter: path"}),
    };
    let include_signature = args
        .get("include_signature")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let abs_path = match resolve_existing_path(file_path, codebase) {
        Ok(path) => path,
        Err(error) => return error,
    };
    if !abs_path.is_file() {
        return json!({"error": format!("Path is not a file: {}", file_path)});
    }

    let parser = TreeSitterParser::new();
    match parser.parse_file(abs_path.to_string_lossy().as_ref()) {
        Ok(parsed) => {
            let mut columns = vec![json!("name"), json!("type"), json!("line")];
            let has_multiline = parsed
                .symbols
                .iter()
                .any(|symbol| symbol.line_end > symbol.line_start + 1);
            if has_multiline {
                columns.push(json!("end_line"));
            }
            if include_signature {
                columns.push(json!("signature"));
            }

            let symbols: Vec<Value> = parsed
                .symbols
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

            json!({
                "file": strip_base(&abs_path.to_string_lossy(), codebase),
                "columns": columns,
                "symbols": symbols,
                "total": symbols.len()
            })
        }
        Err(e) => json!({"error": format!("Parse failed: {}", e)}),
    }
}

fn search_symbols(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
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
        return json!({"error": "Parameter 'symbols' must contain at least one symbol name."});
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

/// Pattern search using grep-style matching with structural awareness.
fn pattern_search(args: &Value, codebase: Option<&str>) -> Value {
    let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
    if pattern.is_empty() {
        return json!({"error": "Missing required parameter: pattern"});
    }
    let language = args.get("language").and_then(|v| v.as_str());
    let file_path = args.get("file_path").and_then(|v| v.as_str());
    let search_path = args.get("path").and_then(|v| v.as_str());

    let target = match resolve_search_target(file_path.or(search_path), codebase) {
        Ok(path) => path,
        Err(error) => return error,
    };

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
    let files = collect_files(target.to_string_lossy().as_ref(), language);

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
    let target = match resolve_search_target(file_path.or(search_path), codebase) {
        Ok(path) => path,
        Err(error) => return error,
    };

    let regex_pattern = pattern_to_regex(pattern);
    let re = match regex_lite::Regex::new(&regex_pattern) {
        Ok(r) => r,
        Err(_) => return json!({"error": "Invalid pattern"}),
    };

    let files = collect_files(target.to_string_lossy().as_ref(), language);
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
    let symbol_name = args
        .get("symbol_name")
        .or_else(|| args.get("name"))
        .and_then(|v| v.as_str());

    let mut target_files: Vec<String> = Vec::new();
    let mut affected_symbols: Vec<Value> = Vec::new();
    let mut risks: Vec<String> = Vec::new();
    let mut seen_symbol_ids = HashSet::new();
    let goal_term_set: HashSet<String> = edit_plan_goal_terms(goal).into_iter().collect();

    let primary_symbols = resolve_edit_plan_primary_symbols(symbol_name, goal, graph);
    let bridge_symbols = expand_edit_plan_bridge_symbols(
        &primary_symbols,
        goal,
        graph,
        &graph.find_nodes_by_name("", false),
        6,
    );

    for node in primary_symbols {
        let file = strip_base(&node.location.file_path, codebase);
        if !target_files.contains(&file) {
            target_files.push(file);
        }
        add_edit_plan_symbol(
            &mut affected_symbols,
            &mut seen_symbol_ids,
            graph,
            &node,
            codebase,
            "primary",
        );
        add_edit_plan_neighbors(
            &mut affected_symbols,
            &mut seen_symbol_ids,
            &mut target_files,
            &mut risks,
            graph,
            &node,
            &goal_term_set,
            codebase,
        );

        let (callers, _) = graph.get_node_degree(&node.id);
        if callers > 5 {
            risks.push(format!(
                "{} has {} callers — high blast radius",
                node.name, callers
            ));
        }
    }

    for node in bridge_symbols {
        let file = strip_base(&node.location.file_path, codebase);
        if !target_files.contains(&file) {
            target_files.push(file);
        }
        add_edit_plan_symbol(
            &mut affected_symbols,
            &mut seen_symbol_ids,
            graph,
            &node,
            codebase,
            "bridge",
        );
    }

    if let Some(fp) = file_path {
        let resolved = resolve_existing_path(fp, codebase)
            .ok()
            .map(|path| strip_base(&path.to_string_lossy(), codebase))
            .unwrap_or_else(|| fp.to_string());
        if !target_files.contains(&resolved) {
            target_files.push(resolved);
        }
    }

    target_files.sort();
    target_files.dedup();
    risks.sort();
    risks.dedup();

    // Find related tests
    let related_tests: Vec<String> = target_files
        .iter()
        .filter_map(|f| {
            let stem = Path::new(f).file_stem()?.to_string_lossy().to_string();
            let test_name = format!("test_{}", stem);
            let matches = graph.find_nodes_by_name(&test_name, false);
            if matches.is_empty() || matches.iter().all(|node| node.name != test_name) {
                None
            } else {
                Some(test_name)
            }
        })
        .collect();

    let mut next_steps = Vec::new();
    if pattern.is_some() {
        next_steps.push("Run pattern_rewrite with dry_run=true before applying edits".to_string());
    }
    if affected_symbols.is_empty() {
        next_steps.push("Resolve the target symbol or file before editing".to_string());
    } else {
        next_steps.push("Review the resolved callers and callees before editing".to_string());
    }
    if !related_tests.is_empty() {
        next_steps.push("Run related tests after applying changes".to_string());
    }

    let confidence = if affected_symbols.is_empty() || target_files.is_empty() {
        "low"
    } else {
        "high"
    };

    json!({
        "goal": goal,
        "target_files": target_files,
        "affected_symbols": affected_symbols,
        "related_tests": related_tests,
        "risks": risks,
        "confidence": confidence,
        "next_steps": next_steps,
    })
}

#[derive(Clone)]
struct CodebaseMapHit {
    node_id: String,
    file: String,
    source_file: String,
    score: f64,
    is_test_like: bool,
    symbol: Value,
}

/// Return a symbol-level map of the codebase grouped by file.
/// Accepts an optional `query` to filter by symbol name and an optional `path` prefix.
fn search_codebase_map(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let raw_query = args
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let normalized_query = raw_query.to_ascii_lowercase();
    let query_tokens = tokenize_codebase_map_text(raw_query);
    let query_term_set: HashSet<String> = query_tokens.iter().cloned().collect();
    let targets_product_surface = codebase_map_query_targets_product_surface(raw_query);
    let prefers_subsystem_closure = codebase_map_query_prefers_subsystem_closure(raw_query);
    let path_filter = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let resolved_filter = if path_filter.is_empty() {
        None
    } else {
        match resolve_existing_path(path_filter, codebase) {
            Ok(path) => Some(path),
            Err(error) => return error,
        }
    };
    let filter_is_dir = resolved_filter
        .as_ref()
        .map(|path| path.is_dir())
        .unwrap_or(false);

    let all_nodes = graph.find_nodes_by_name("", false);
    let mut hits = Vec::new();
    for node in &all_nodes {
        if let Some(filter_path) = resolved_filter.as_ref() {
            if !path_matches(&node.location.file_path, filter_path, filter_is_dir) {
                continue;
            }
        }
        let mut score = codebase_map_match_score(node, &normalized_query, &query_tokens);
        if targets_product_surface {
            score += codebase_map_surface_bias(node);
        }
        if !normalized_query.is_empty() && score <= 0.0 {
            continue;
        }
        let rel = strip_base(&node.location.file_path, codebase);
        let (callers, callees) = graph.get_node_degree(&node.id);
        hits.push(CodebaseMapHit {
            node_id: node.id.clone(),
            is_test_like: is_test_file(&node.location.file_path)
                || is_probable_codebase_map_test_symbol(&node.name),
            file: rel,
            source_file: node.location.file_path.clone(),
            score,
            symbol: json!({
                "name": node.name,
                "type": node.node_type.to_string(),
                "line": node.location.start_line,
                "callers": callers,
                "callees": callees,
            }),
        });
    }

    let mut subsystem_dominant_file: Option<String> = None;

    if !normalized_query.is_empty() {
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.file.cmp(&b.file))
                .then_with(|| {
                    a.symbol["line"]
                        .as_u64()
                        .unwrap_or(0)
                        .cmp(&b.symbol["line"].as_u64().unwrap_or(0))
                })
        });

        let dominant_file =
            detect_dominant_codebase_map_file(&hits, graph, &query_tokens, targets_product_surface);
        if prefers_subsystem_closure {
            subsystem_dominant_file = dominant_file.clone();
        }

        let mut seed_ids = Vec::new();
        if let Some(dominant_file) = dominant_file.as_ref() {
            let mut dominant_hits: Vec<&CodebaseMapHit> = hits
                .iter()
                .filter(|hit| &hit.source_file == dominant_file)
                .collect();
            dominant_hits.sort_by(|a, b| {
                let a_score = graph
                    .get_node(&a.node_id)
                    .map(|node| {
                        codebase_map_intra_file_relevance_score(
                            &node,
                            &normalized_query,
                            &query_tokens,
                            targets_product_surface,
                        ) + codebase_map_local_connectivity_bias(graph, &node)
                    })
                    .unwrap_or(a.score);
                let b_score = graph
                    .get_node(&b.node_id)
                    .map(|node| {
                        codebase_map_intra_file_relevance_score(
                            &node,
                            &normalized_query,
                            &query_tokens,
                            targets_product_surface,
                        ) + codebase_map_local_connectivity_bias(graph, &node)
                    })
                    .unwrap_or(b.score);
                b_score
                    .partial_cmp(&a_score)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| {
                        a.symbol["line"]
                            .as_u64()
                            .unwrap_or(0)
                            .cmp(&b.symbol["line"].as_u64().unwrap_or(0))
                    })
            });

            for hit in dominant_hits.into_iter().take(3) {
                seed_ids.push(hit.node_id.clone());
            }
        }
        for hit in &hits {
            if seed_ids.len() >= 3 {
                break;
            }
            if !seed_ids.iter().any(|id| id == &hit.node_id) {
                seed_ids.push(hit.node_id.clone());
            }
        }

        if let Some(dominant_file) = dominant_file.as_ref() {
            if prefers_subsystem_closure {
                let (subsystem_hits, _) = collect_dominant_file_subsystem_hits(
                    &hits,
                    graph,
                    dominant_file,
                    &normalized_query,
                    &query_tokens,
                    targets_product_surface,
                    codebase,
                );
                hits.extend(subsystem_hits);
            }
        }

        let mut expanded_hits = Vec::new();

        for seed_id in seed_ids {
            let Some(seed) = graph.get_node(&seed_id) else {
                continue;
            };

            let seed_file = seed.location.file_path.clone();
            let mut seen_neighbors = HashSet::new();
            let mut neighbors: Vec<UniversalNode> = graph
                .get_callers(&seed_id)
                .into_iter()
                .chain(graph.get_callees(&seed_id).into_iter())
                .filter(|node| seen_neighbors.insert(node.id.clone()))
                .collect();
            neighbors.sort_by(|a, b| {
                let a_score = codebase_map_expansion_score(
                    a,
                    &normalized_query,
                    &query_tokens,
                    &seed_file,
                    dominant_file.as_deref(),
                    targets_product_surface,
                );
                let b_score = codebase_map_expansion_score(
                    b,
                    &normalized_query,
                    &query_tokens,
                    &seed_file,
                    dominant_file.as_deref(),
                    targets_product_surface,
                );
                b_score
                    .partial_cmp(&a_score)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| a.location.start_line.cmp(&b.location.start_line))
            });

            for node in neighbors.into_iter().take(4) {
                if !should_keep_codebase_map_neighbor(
                    &node,
                    &query_term_set,
                    dominant_file.as_deref(),
                    &seed_file,
                ) {
                    continue;
                }
                let score = codebase_map_expansion_score(
                    &node,
                    &normalized_query,
                    &query_tokens,
                    &seed_file,
                    dominant_file.as_deref(),
                    targets_product_surface,
                );
                let rel = strip_base(&node.location.file_path, codebase);
                let (callers, callees) = graph.get_node_degree(&node.id);
                expanded_hits.push(CodebaseMapHit {
                    node_id: node.id.clone(),
                    is_test_like: is_test_file(&node.location.file_path)
                        || is_probable_codebase_map_test_symbol(&node.name),
                    file: rel,
                    source_file: node.location.file_path.clone(),
                    score,
                    symbol: json!({
                        "name": node.name,
                        "type": node.node_type.to_string(),
                        "line": node.location.start_line,
                        "callers": callers,
                        "callees": callees,
                    }),
                });
            }
        }

        hits.extend(expanded_hits);

        if let Some(dominant_file) = dominant_file.as_ref() {
            if !prefers_subsystem_closure {
                let dominant_concepts: HashSet<String> = hits
                    .iter()
                    .filter(|hit| &hit.source_file == dominant_file)
                    .take(6)
                    .filter_map(|hit| graph.get_node(&hit.node_id))
                    .flat_map(|node| codebase_map_symbol_candidate_tokens(&node))
                    .chain(query_tokens.iter().cloned())
                    .collect();

                let mut same_file_candidates: Vec<(UniversalNode, f64)> = all_nodes
                    .iter()
                    .filter(|node| node.location.file_path == *dominant_file)
                    .filter(|node| {
                        should_keep_same_file_codebase_map_candidate(
                            node,
                            &query_term_set,
                            &dominant_concepts,
                        )
                    })
                    .map(|node| {
                        (
                            node.clone(),
                            codebase_map_same_file_score(
                                node,
                                &normalized_query,
                                &query_tokens,
                                &dominant_concepts,
                                targets_product_surface,
                                codebase_map_local_connectivity_bias(graph, node),
                            ),
                        )
                    })
                    .filter(|(_, score)| *score >= 0.50)
                    .collect();
                same_file_candidates.sort_by(|a, b| {
                    b.1.partial_cmp(&a.1)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| a.0.location.start_line.cmp(&b.0.location.start_line))
                });

                for (node, score) in same_file_candidates.into_iter().take(8) {
                    let rel = strip_base(&node.location.file_path, codebase);
                    let (callers, callees) = graph.get_node_degree(&node.id);
                    hits.push(CodebaseMapHit {
                        node_id: node.id.clone(),
                        is_test_like: is_test_file(&node.location.file_path)
                            || is_probable_codebase_map_test_symbol(&node.name),
                        file: rel,
                        source_file: node.location.file_path.clone(),
                        score,
                        symbol: json!({
                            "name": node.name,
                            "type": node.node_type.to_string(),
                            "line": node.location.start_line,
                            "callers": callers,
                            "callees": callees,
                        }),
                    });
                }
            }
        }
    }

    if !codebase_map_query_targets_tests(raw_query) && hits.iter().any(|hit| !hit.is_test_like) {
        hits.retain(|hit| !hit.is_test_like);
    }

    let mut deduped_hits: HashMap<String, CodebaseMapHit> = HashMap::new();
    for hit in hits {
        match deduped_hits.get_mut(&hit.node_id) {
            Some(existing) if existing.score >= hit.score => {}
            Some(existing) => *existing = hit,
            None => {
                deduped_hits.insert(hit.node_id.clone(), hit);
            }
        }
    }
    let mut hits: Vec<CodebaseMapHit> = deduped_hits.into_values().collect();

    if !normalized_query.is_empty() {
        let dominant_file = if prefers_subsystem_closure {
            subsystem_dominant_file.clone().or_else(|| {
                detect_dominant_codebase_map_file(
                    &hits,
                    graph,
                    &query_tokens,
                    targets_product_surface,
                )
            })
        } else {
            detect_dominant_codebase_map_file(&hits, graph, &query_tokens, targets_product_surface)
        };

        if let Some(dominant_file) = dominant_file.as_ref() {
            let subsystem_nodes = if prefers_subsystem_closure {
                build_dominant_file_subsystem_nodes(
                    &hits,
                    graph,
                    dominant_file,
                    &normalized_query,
                    &query_tokens,
                    targets_product_surface,
                )
            } else {
                Vec::new()
            };
            let subsystem_ids: HashSet<String> = subsystem_nodes
                .iter()
                .map(|(node, _, _)| node.id.clone())
                .collect();
            let dominant_concepts: HashSet<String> =
                if prefers_subsystem_closure && !subsystem_nodes.is_empty() {
                    subsystem_nodes
                        .iter()
                        .flat_map(|(node, _, _)| codebase_map_symbol_candidate_tokens(node))
                        .chain(query_tokens.iter().cloned())
                        .collect()
                } else {
                    hits.iter()
                        .filter(|hit| &hit.source_file == dominant_file)
                        .take(8)
                        .filter_map(|hit| graph.get_node(&hit.node_id))
                        .flat_map(|node| codebase_map_symbol_candidate_tokens(&node))
                        .chain(query_tokens.iter().cloned())
                        .collect()
                };

            for hit in &mut hits {
                let Some(node) = graph.get_node(&hit.node_id) else {
                    continue;
                };
                let concept_overlap = if &hit.source_file == dominant_file {
                    codebase_map_symbol_concept_overlap(&node, &dominant_concepts) as f64
                } else {
                    codebase_map_concept_overlap(&node, &dominant_concepts) as f64
                };
                if &hit.source_file == dominant_file {
                    hit.score += 0.18 + concept_overlap.min(3.0) * 0.05;
                } else if concept_overlap == 0.0 {
                    hit.score = (hit.score - 0.25).max(0.0);
                } else if concept_overlap < 2.0 {
                    hit.score = (hit.score - 0.10).max(0.0);
                }
            }

            if prefers_subsystem_closure {
                apply_dominant_file_focus(
                    &mut hits,
                    graph,
                    dominant_file,
                    &dominant_concepts,
                    Some(&subsystem_ids),
                );
            }
        }
    }

    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| {
                a.symbol["line"]
                    .as_u64()
                    .unwrap_or(0)
                    .cmp(&b.symbol["line"].as_u64().unwrap_or(0))
            })
    });

    if !normalized_query.is_empty() {
        let top_score = hits.first().map(|hit| hit.score).unwrap_or(0.0);
        let min_score = (top_score * 0.45).max(0.5);
        hits.retain(|hit| hit.score >= min_score);
    }

    let mut grouped: Vec<(String, Vec<CodebaseMapHit>, f64)> = Vec::new();
    for hit in hits {
        if let Some((_, symbols, top_score)) =
            grouped.iter_mut().find(|(file, _, _)| *file == hit.file)
        {
            *top_score = top_score.max(hit.score);
            symbols.push(hit);
        } else {
            let top_score = hit.score;
            grouped.push((hit.file.clone(), vec![hit], top_score));
        }
    }

    if normalized_query.is_empty() {
        grouped.sort_by(|a, b| a.0.cmp(&b.0));
    } else {
        grouped.sort_by(|a, b| {
            b.2.partial_cmp(&a.2)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });

        if codebase_map_query_is_narrow_explanatory(raw_query, &query_tokens) && grouped.len() > 1 {
            let lead_score = grouped[0].2;
            let second_score = grouped[1].2;
            let max_files = if lead_score >= second_score + 0.35 {
                1
            } else {
                2
            };
            let retain_floor = if max_files == 1 {
                lead_score - 0.01
            } else {
                (lead_score - 0.18).max(0.5)
            };

            grouped.retain(|(_, _, top_score)| *top_score >= retain_floor);
            grouped.truncate(max_files);
        }
    }

    let files: Vec<Value> = grouped
        .into_iter()
        .take(if normalized_query.is_empty() {
            usize::MAX
        } else {
            5
        })
        .map(|(file, mut symbols, _)| {
            if normalized_query.is_empty() {
                symbols.sort_by(|a, b| {
                    a.symbol["line"]
                        .as_u64()
                        .unwrap_or(0)
                        .cmp(&b.symbol["line"].as_u64().unwrap_or(0))
                });
            } else {
                symbols.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| {
                            a.symbol["line"]
                                .as_u64()
                                .unwrap_or(0)
                                .cmp(&b.symbol["line"].as_u64().unwrap_or(0))
                        })
                });
                symbols.truncate(8);
            }
            let total = symbols.len();
            json!({
                "file": file,
                "symbols": symbols.into_iter().map(|hit| hit.symbol).collect::<Vec<_>>(),
                "total": total
            })
        })
        .collect();

    let total_symbols: usize = files
        .iter()
        .map(|f| f["total"].as_u64().unwrap_or(0) as usize)
        .sum();

    json!({
        "path": if let Some(path) = resolved_filter.as_ref() {
            json!(strip_base(&path.to_string_lossy(), codebase))
        } else {
            json!(".")
        },
        "query": if raw_query.is_empty() {
            Value::Null
        } else {
            json!(raw_query)
        },
        "files": files,
        "total_files": files.len(),
        "total_symbols": total_symbols,
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn strip_base(file: &str, codebase: Option<&str>) -> String {
    if let Some(base) = codebase {
        let file_path = Path::new(file);
        if let Ok(stripped) = file_path.strip_prefix(base) {
            return stripped.to_string_lossy().to_string();
        }

        let canonical_file = std::fs::canonicalize(file_path).ok();
        let canonical_base = std::fs::canonicalize(base).ok();
        if let (Some(canonical_file), Some(canonical_base)) = (canonical_file, canonical_base) {
            if let Ok(stripped) = canonical_file.strip_prefix(&canonical_base) {
                return stripped.to_string_lossy().to_string();
            }
        }
    }

    file.to_string()
}

fn codebase_map_match_score(
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
) -> f64 {
    codebase_map_match_score_with_path(node, normalized_query, query_tokens, true)
}

fn codebase_map_symbol_match_score(
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
) -> f64 {
    codebase_map_match_score_with_path(node, normalized_query, query_tokens, false)
}

fn codebase_map_match_score_with_path(
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
    include_file_path: bool,
) -> f64 {
    if normalized_query.is_empty() {
        return 1.0;
    }

    let exact_match = codebase_map_exact_query_match(node, normalized_query, include_file_path);
    let candidate_tokens = codebase_map_candidate_tokens_with_path(node, include_file_path);
    let matched_terms = query_tokens
        .iter()
        .filter(|term| {
            candidate_tokens.iter().any(|candidate| {
                candidate == *term
                    || candidate.contains(term.as_str())
                    || term.contains(candidate.as_str())
            })
        })
        .count();

    if !exact_match && matched_terms < required_codebase_map_matches(query_tokens.len()) {
        return 0.0;
    }

    let overlap = if query_tokens.is_empty() {
        0.0
    } else {
        matched_terms as f64 / query_tokens.len() as f64
    };
    let exact_bonus = if exact_match { 1.0 } else { 0.0 };
    let content_bonus = if !node.content.is_empty()
        && node.content.to_ascii_lowercase().contains(normalized_query)
    {
        0.2
    } else {
        0.0
    };

    exact_bonus + overlap + content_bonus
}

fn codebase_map_intra_file_relevance_score(
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
    targets_product_surface: bool,
) -> f64 {
    codebase_map_symbol_match_score(node, normalized_query, query_tokens)
        + if targets_product_surface {
            codebase_map_surface_bias(node)
        } else {
            0.0
        }
        + codebase_map_subsystem_role_bias(node, targets_product_surface)
        - codebase_map_local_meta_helper_penalty(node, targets_product_surface)
}

fn codebase_map_expansion_score(
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
    seed_file: &str,
    dominant_file: Option<&str>,
    targets_product_surface: bool,
) -> f64 {
    let base = codebase_map_match_score(node, normalized_query, query_tokens)
        + if targets_product_surface {
            codebase_map_surface_bias(node)
        } else {
            0.0
        };
    let same_seed_file = node.location.file_path == seed_file;
    let same_dominant_file = dominant_file
        .map(|file| node.location.file_path == file)
        .unwrap_or(false);
    let helper_penalty = if is_codebase_map_generic_helper_symbol(&node.name)
        && !same_seed_file
        && !same_dominant_file
    {
        0.15
    } else {
        0.0
    };

    base + codebase_map_subsystem_role_bias(node, targets_product_surface)
        + 0.20
        + if same_seed_file { 0.32 } else { 0.0 }
        + if same_dominant_file { 0.16 } else { 0.0 }
        - helper_penalty
}

fn codebase_map_same_file_score(
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
    concept_tokens: &HashSet<String>,
    targets_product_surface: bool,
    connectivity_bias: f64,
) -> f64 {
    let base = codebase_map_intra_file_relevance_score(
        node,
        normalized_query,
        query_tokens,
        targets_product_surface,
    );
    let concept_overlap = codebase_map_symbol_concept_overlap(node, concept_tokens) as f64;
    let exact_name_bonus = if node.name.to_ascii_lowercase().contains(normalized_query) {
        0.2
    } else {
        0.0
    };
    base + connectivity_bias + concept_overlap.min(4.0) * 0.22 + 0.20 + exact_name_bonus
}

fn should_keep_codebase_map_neighbor(
    node: &UniversalNode,
    query_terms: &HashSet<String>,
    dominant_file: Option<&str>,
    seed_file: &str,
) -> bool {
    let query_overlap = codebase_map_concept_overlap(node, query_terms);
    if is_codebase_map_meta_helper_symbol(&node.name) && query_overlap < 2 {
        return false;
    }
    if query_overlap > 0 {
        return true;
    }

    let same_seed_file = node.location.file_path == seed_file;
    let same_dominant_file = dominant_file
        .map(|file| node.location.file_path == file)
        .unwrap_or(false);

    !(same_seed_file || same_dominant_file) || !is_codebase_map_generic_helper_symbol(&node.name)
}

fn should_keep_same_file_codebase_map_candidate(
    node: &UniversalNode,
    query_terms: &HashSet<String>,
    dominant_concepts: &HashSet<String>,
) -> bool {
    let query_overlap = codebase_map_symbol_concept_overlap(node, query_terms);
    if is_codebase_map_meta_helper_symbol(&node.name) && query_overlap < 2 {
        return false;
    }
    if query_overlap > 0 {
        return true;
    }

    let dominant_overlap = codebase_map_symbol_concept_overlap(node, dominant_concepts);
    dominant_overlap >= 3 && !is_codebase_map_meta_helper_symbol(&node.name)
}

fn detect_dominant_codebase_map_file(
    hits: &[CodebaseMapHit],
    graph: &CodeGraph,
    query_tokens: &[String],
    targets_product_surface: bool,
) -> Option<String> {
    #[derive(Clone, Copy, Default)]
    struct FileStats {
        hit_count: usize,
        total_score: f64,
        concept_overlap: usize,
        product_surface_hits: usize,
    }

    let mut file_scores: HashMap<&str, FileStats> = HashMap::new();
    let concept_terms: HashSet<String> = query_tokens.iter().cloned().collect();

    for hit in hits.iter().take(12) {
        let entry = file_scores.entry(hit.source_file.as_str()).or_default();
        entry.hit_count += 1;
        entry.total_score += hit.score;
        if let Some(node) = graph.get_node(&hit.node_id) {
            entry.concept_overlap += codebase_map_concept_overlap(&node, &concept_terms);
            if targets_product_surface && is_probable_codebase_map_product_surface_node(&node) {
                entry.product_surface_hits += 1;
            }
        }
    }

    let mut ranked_files: Vec<(&str, f64, FileStats)> = file_scores
        .iter()
        .map(|(file, stats)| {
            let weighted_score = stats.total_score
                + stats.hit_count as f64 * 0.22
                + stats.concept_overlap.min(8) as f64 * 0.08
                + if targets_product_surface {
                    stats.product_surface_hits as f64 * 0.10
                } else {
                    0.0
                };
            (*file, weighted_score, *stats)
        })
        .collect();
    ranked_files.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| b.2.hit_count.cmp(&a.2.hit_count))
            .then_with(|| a.0.cmp(b.0))
    });

    match ranked_files.as_slice() {
        [(file, weighted_score, stats), (.., second_score, _)]
            if stats.hit_count >= 3 && *weighted_score >= *second_score + 0.40 =>
        {
            Some((*file).to_string())
        }
        [(file, _, stats)] if stats.hit_count >= 3 => Some((*file).to_string()),
        _ => None,
    }
}

fn codebase_map_query_prefers_subsystem_closure(query: &str) -> bool {
    let trimmed = query.trim();
    !trimmed.is_empty()
        && trimmed.split_whitespace().count() >= 3
        && !codebase_map_query_targets_tests(query)
}

fn codebase_map_query_is_narrow_explanatory(query: &str, query_tokens: &[String]) -> bool {
    let lowered = query.to_ascii_lowercase();
    let has_explanatory_prefix = lowered.contains("how does")
        || lowered.contains("how do")
        || lowered.contains("what does")
        || lowered.starts_with("explain ");

    has_explanatory_prefix && !query_tokens.is_empty() && query_tokens.len() <= 4
}

fn apply_dominant_file_focus(
    hits: &mut Vec<CodebaseMapHit>,
    graph: &CodeGraph,
    dominant_file: &str,
    dominant_concepts: &HashSet<String>,
    subsystem_ids: Option<&HashSet<String>>,
) {
    let dominant_scores: Vec<f64> = hits
        .iter()
        .filter(|hit| hit.source_file == dominant_file)
        .map(|hit| hit.score)
        .collect();
    let same_file_hits = dominant_scores.len();
    if same_file_hits < 4 {
        return;
    }

    let dominant_top_score = dominant_scores.iter().copied().fold(0.0_f64, f64::max);
    let dominant_floor = dominant_scores
        .iter()
        .copied()
        .reduce(f64::min)
        .unwrap_or(0.0);

    hits.retain(|hit| {
        if hit.source_file == dominant_file {
            if let Some(subsystem_ids) = subsystem_ids {
                if !subsystem_ids.is_empty() {
                    return subsystem_ids.contains(&hit.node_id);
                }
            }
            return true;
        }

        let Some(node) = graph.get_node(&hit.node_id) else {
            return false;
        };
        let concept_overlap = codebase_map_concept_overlap(&node, dominant_concepts);
        let near_dominant = hit.score >= dominant_top_score * 0.90;
        let solid_floor = hit.score >= dominant_floor + 0.12;

        concept_overlap >= 3 && near_dominant && solid_floor
    });
}

fn codebase_map_concept_overlap(node: &UniversalNode, concept_tokens: &HashSet<String>) -> usize {
    let candidate_tokens = codebase_map_candidate_tokens(node);
    concept_tokens
        .iter()
        .filter(|term| {
            candidate_tokens.iter().any(|candidate| {
                candidate == *term
                    || candidate.contains(term.as_str())
                    || term.contains(candidate.as_str())
            })
        })
        .count()
}

fn codebase_map_symbol_concept_overlap(
    node: &UniversalNode,
    concept_tokens: &HashSet<String>,
) -> usize {
    let candidate_tokens = codebase_map_symbol_candidate_tokens(node);
    concept_tokens
        .iter()
        .filter(|term| {
            candidate_tokens.iter().any(|candidate| {
                candidate == *term
                    || candidate.contains(term.as_str())
                    || term.contains(candidate.as_str())
            })
        })
        .count()
}

fn codebase_map_candidate_tokens(node: &UniversalNode) -> HashSet<String> {
    codebase_map_candidate_tokens_with_path(node, true)
}

fn codebase_map_symbol_candidate_tokens(node: &UniversalNode) -> HashSet<String> {
    codebase_map_candidate_tokens_with_path(node, false)
}

fn codebase_map_candidate_tokens_with_path(
    node: &UniversalNode,
    include_file_path: bool,
) -> HashSet<String> {
    let qualified_name = node
        .parent
        .as_ref()
        .map(|parent| format!("{parent}.{}", node.name))
        .unwrap_or_else(|| node.name.clone());
    let mut tokens: HashSet<String> = [
        node.name.as_str(),
        qualified_name.as_str(),
        node.content.as_str(),
        node.docstring.as_deref().unwrap_or(""),
    ]
    .into_iter()
    .flat_map(tokenize_codebase_map_text)
    .collect();
    if include_file_path {
        tokens.extend(tokenize_codebase_map_text(&node.location.file_path));
    }
    tokens
}

fn codebase_map_exact_query_match(
    node: &UniversalNode,
    normalized_query: &str,
    include_file_path: bool,
) -> bool {
    let qualified_name = node
        .parent
        .as_ref()
        .map(|parent| format!("{parent}.{}", node.name))
        .unwrap_or_else(|| node.name.clone());
    let mut fields = vec![
        node.name.as_str(),
        qualified_name.as_str(),
        node.content.as_str(),
        node.docstring.as_deref().unwrap_or(""),
    ];
    if include_file_path {
        fields.push(node.location.file_path.as_str());
    }
    fields
        .into_iter()
        .any(|field| field.to_ascii_lowercase().contains(normalized_query))
}

fn is_codebase_map_generic_helper_symbol(symbol_name: &str) -> bool {
    let symbol_name = symbol_name
        .rsplit("::")
        .next()
        .unwrap_or(symbol_name)
        .rsplit('.')
        .next()
        .unwrap_or(symbol_name)
        .to_ascii_lowercase();

    symbol_name.starts_with("resolve_")
        || symbol_name.starts_with("normalize_")
        || symbol_name.starts_with("tokenize_")
        || symbol_name.starts_with("collect_")
        || symbol_name.ends_with("_by_degree")
}

fn is_codebase_map_meta_helper_symbol(symbol_name: &str) -> bool {
    let symbol_name = symbol_name
        .rsplit("::")
        .next()
        .unwrap_or(symbol_name)
        .rsplit('.')
        .next()
        .unwrap_or(symbol_name)
        .to_ascii_lowercase();

    symbol_name.starts_with("query_targets_")
        || symbol_name.starts_with("confidence_")
        || symbol_name == "accumulate_result"
        || symbol_name == "fuse_results"
        || symbol_name == "rerank_result_limit"
}

fn codebase_map_subsystem_role_bias(node: &UniversalNode, targets_product_surface: bool) -> f64 {
    if !targets_product_surface {
        return 0.0;
    }

    let symbol_name = node
        .name
        .rsplit("::")
        .next()
        .unwrap_or(&node.name)
        .rsplit('.')
        .next()
        .unwrap_or(&node.name)
        .to_ascii_lowercase();

    if is_codebase_map_meta_helper_symbol(&symbol_name) {
        -0.45
    } else if symbol_name.starts_with("handle_") {
        0.22
    } else if symbol_name.starts_with("rerank_")
        || symbol_name.contains("_rerank")
        || symbol_name.starts_with("drop_")
        || symbol_name.starts_with("filter_")
        || symbol_name.contains("_filter")
        || symbol_name.contains("classifier")
        || symbol_name.starts_with("classify_")
        || symbol_name.contains("guard")
    {
        0.18
    } else if symbol_name.starts_with("is_")
        && (symbol_name.contains("query")
            || symbol_name.contains("lookup")
            || symbol_name.contains("symbol"))
    {
        0.16
    } else if symbol_name.contains("match")
        && (symbol_name.contains("query")
            || symbol_name.contains("symbol")
            || symbol_name.contains("result"))
    {
        0.16
    } else if symbol_name.ends_with("_limit")
        && (symbol_name.contains("candidate") || symbol_name.contains("result"))
    {
        0.16
    } else {
        0.0
    }
}

fn codebase_map_has_behavioral_role(node: &UniversalNode) -> bool {
    codebase_map_subsystem_role_bias(node, true) > 0.0
}

fn codebase_map_anchor_score(
    graph: &CodeGraph,
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
    targets_product_surface: bool,
) -> f64 {
    codebase_map_symbol_match_score(node, normalized_query, query_tokens)
        + if targets_product_surface {
            codebase_map_surface_bias(node)
        } else {
            0.0
        }
        + codebase_map_subsystem_role_bias(node, targets_product_surface)
        + codebase_map_local_connectivity_bias(graph, node)
        - codebase_map_local_meta_helper_penalty(node, targets_product_surface)
        - if is_codebase_map_generic_helper_symbol(&node.name)
            && !codebase_map_has_behavioral_role(node)
        {
            0.18
        } else {
            0.0
        }
}

fn codebase_map_subsystem_closure_score(
    graph: &CodeGraph,
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
    targets_product_surface: bool,
    distance: usize,
) -> f64 {
    let distance_bonus = match distance {
        0 => 0.40,
        1 => 0.24,
        2 => 0.12,
        _ => 0.0,
    };
    let role_bonus = if distance > 0 && codebase_map_has_behavioral_role(node) {
        0.08
    } else {
        0.0
    };

    codebase_map_anchor_score(
        graph,
        node,
        normalized_query,
        query_tokens,
        targets_product_surface,
    ) + distance_bonus
        + role_bonus
}

fn select_dominant_file_subsystem_anchors(
    hits: &[CodebaseMapHit],
    graph: &CodeGraph,
    dominant_file: &str,
    normalized_query: &str,
    query_tokens: &[String],
    targets_product_surface: bool,
) -> Vec<String> {
    let mut anchor_candidates: Vec<(String, f64, u32)> = hits
        .iter()
        .filter(|hit| hit.source_file == dominant_file)
        .filter_map(|hit| {
            let node = graph.get_node(&hit.node_id)?;
            if is_codebase_map_meta_helper_symbol(&node.name)
                || hit.is_test_like
                || is_codebase_map_generic_helper_symbol(&node.name)
                    && !codebase_map_has_behavioral_role(&node)
            {
                return None;
            }

            let score = codebase_map_anchor_score(
                graph,
                &node,
                normalized_query,
                query_tokens,
                targets_product_surface,
            );
            if score < 0.55 {
                return None;
            }

            Some((node.id.clone(), score, node.location.start_line))
        })
        .collect();

    anchor_candidates.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.2.cmp(&b.2))
    });
    anchor_candidates.dedup_by(|a, b| a.0 == b.0);
    anchor_candidates
        .into_iter()
        .take(3)
        .map(|(node_id, _, _)| node_id)
        .collect()
}

fn should_include_subsystem_closure_node(
    graph: &CodeGraph,
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
    query_terms: &HashSet<String>,
    targets_product_surface: bool,
    distance: usize,
) -> bool {
    if is_codebase_map_meta_helper_symbol(&node.name)
        || is_probable_codebase_map_test_symbol(&node.name)
    {
        return false;
    }

    let score = codebase_map_subsystem_closure_score(
        graph,
        node,
        normalized_query,
        query_tokens,
        targets_product_surface,
        distance,
    );
    let query_overlap = codebase_map_symbol_concept_overlap(node, query_terms);
    let behavioral_role = codebase_map_has_behavioral_role(node);
    let generic_non_behavioral =
        is_codebase_map_generic_helper_symbol(&node.name) && !behavioral_role;
    if generic_non_behavioral && query_overlap == 0 {
        return false;
    }

    match distance {
        0 => score >= 0.55,
        1 => score >= 0.78 && (behavioral_role || query_overlap >= 1),
        2 => score >= 0.82 && behavioral_role && query_overlap >= 1,
        _ => false,
    }
}

fn should_expand_subsystem_closure_from(
    graph: &CodeGraph,
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
    query_terms: &HashSet<String>,
    targets_product_surface: bool,
    distance: usize,
) -> bool {
    if distance >= 2 {
        return false;
    }

    let score = codebase_map_subsystem_closure_score(
        graph,
        node,
        normalized_query,
        query_tokens,
        targets_product_surface,
        distance,
    );
    let query_overlap = codebase_map_symbol_concept_overlap(node, query_terms);

    codebase_map_has_behavioral_role(node) || query_overlap >= 2 || score >= 1.15
}

fn build_dominant_file_subsystem_nodes(
    hits: &[CodebaseMapHit],
    graph: &CodeGraph,
    dominant_file: &str,
    normalized_query: &str,
    query_tokens: &[String],
    targets_product_surface: bool,
) -> Vec<(UniversalNode, usize, f64)> {
    let anchor_ids = select_dominant_file_subsystem_anchors(
        hits,
        graph,
        dominant_file,
        normalized_query,
        query_tokens,
        targets_product_surface,
    );
    if anchor_ids.is_empty() {
        return Vec::new();
    }

    let query_terms: HashSet<String> = query_tokens.iter().cloned().collect();
    let mut queue: VecDeque<(String, usize)> = anchor_ids
        .iter()
        .cloned()
        .map(|node_id| (node_id, 0))
        .collect();
    let mut best_nodes: HashMap<String, (usize, f64)> = HashMap::new();

    for anchor_id in &anchor_ids {
        let Some(anchor) = graph.get_node(anchor_id) else {
            continue;
        };
        let score = codebase_map_subsystem_closure_score(
            graph,
            &anchor,
            normalized_query,
            query_tokens,
            targets_product_surface,
            0,
        );
        best_nodes.insert(anchor.id.clone(), (0, score));
    }

    while let Some((node_id, distance)) = queue.pop_front() {
        let Some(node) = graph.get_node(&node_id) else {
            continue;
        };
        if node.location.file_path != dominant_file || distance >= 2 {
            continue;
        }

        let mut seen_neighbors = HashSet::new();
        let neighbors: Vec<UniversalNode> = graph
            .get_callers(&node_id)
            .into_iter()
            .chain(graph.get_callees(&node_id).into_iter())
            .filter(|neighbor| {
                neighbor.location.file_path == dominant_file
                    && seen_neighbors.insert(neighbor.id.clone())
            })
            .collect();

        for neighbor in neighbors {
            let next_distance = distance + 1;
            if !should_include_subsystem_closure_node(
                graph,
                &neighbor,
                normalized_query,
                query_tokens,
                &query_terms,
                targets_product_surface,
                next_distance,
            ) {
                continue;
            }

            let score = codebase_map_subsystem_closure_score(
                graph,
                &neighbor,
                normalized_query,
                query_tokens,
                targets_product_surface,
                next_distance,
            );

            let should_update = match best_nodes.get(&neighbor.id) {
                Some((best_distance, best_score)) => {
                    next_distance < *best_distance
                        || (next_distance == *best_distance && score > *best_score)
                }
                None => true,
            };

            if should_update {
                best_nodes.insert(neighbor.id.clone(), (next_distance, score));
            }

            if should_expand_subsystem_closure_from(
                graph,
                &neighbor,
                normalized_query,
                query_tokens,
                &query_terms,
                targets_product_surface,
                next_distance,
            ) {
                queue.push_back((neighbor.id.clone(), next_distance));
            }
        }
    }

    let mut subsystem_nodes: Vec<(UniversalNode, usize, f64)> = best_nodes
        .into_iter()
        .filter_map(|(node_id, (distance, score))| {
            graph.get_node(&node_id).map(|node| (node, distance, score))
        })
        .collect();
    subsystem_nodes.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.0.location.start_line.cmp(&b.0.location.start_line))
    });
    subsystem_nodes.truncate(8);
    subsystem_nodes
}

fn collect_dominant_file_subsystem_hits(
    hits: &[CodebaseMapHit],
    graph: &CodeGraph,
    dominant_file: &str,
    normalized_query: &str,
    query_tokens: &[String],
    targets_product_surface: bool,
    codebase: Option<&str>,
) -> (Vec<CodebaseMapHit>, HashSet<String>) {
    let subsystem_nodes = build_dominant_file_subsystem_nodes(
        hits,
        graph,
        dominant_file,
        normalized_query,
        query_tokens,
        targets_product_surface,
    );
    let subsystem_ids: HashSet<String> = subsystem_nodes
        .iter()
        .map(|(node, _, _)| node.id.clone())
        .collect();
    let subsystem_hits = subsystem_nodes
        .into_iter()
        .map(|(node, _, score)| {
            let rel = strip_base(&node.location.file_path, codebase);
            let (callers, callees) = graph.get_node_degree(&node.id);
            CodebaseMapHit {
                node_id: node.id.clone(),
                is_test_like: is_test_file(&node.location.file_path)
                    || is_probable_codebase_map_test_symbol(&node.name),
                file: rel,
                source_file: node.location.file_path.clone(),
                score,
                symbol: json!({
                    "name": node.name,
                    "type": node.node_type.to_string(),
                    "line": node.location.start_line,
                    "callers": callers,
                    "callees": callees,
                }),
            }
        })
        .collect();

    (subsystem_hits, subsystem_ids)
}

fn codebase_map_local_meta_helper_penalty(
    node: &UniversalNode,
    targets_product_surface: bool,
) -> f64 {
    let symbol_name = node
        .name
        .rsplit("::")
        .next()
        .unwrap_or(&node.name)
        .rsplit('.')
        .next()
        .unwrap_or(&node.name)
        .to_ascii_lowercase();

    if is_codebase_map_meta_helper_symbol(&symbol_name) {
        if targets_product_surface {
            0.72
        } else {
            0.42
        }
    } else {
        0.0
    }
}

fn codebase_map_local_connectivity_bias(graph: &CodeGraph, node: &UniversalNode) -> f64 {
    let (callers, callees) = graph.get_node_degree(&node.id);
    let total_degree = callers + callees;
    let shared_flow_bonus = if callers > 0 && callees > 0 {
        0.08
    } else {
        0.0
    };

    total_degree.min(4) as f64 * 0.07 + shared_flow_bonus
}

fn codebase_map_query_targets_product_surface(query: &str) -> bool {
    let lowered = query.to_ascii_lowercase();
    if codebase_map_query_targets_engine_internals(&lowered) {
        return false;
    }

    lowered.contains("how does")
        || lowered.contains("how do")
        || [
            "mcp",
            "noise",
            "output",
            "persist",
            "persistence",
            "ranking",
            "response",
            "surface",
            "tool",
            "workflow",
            "work",
        ]
        .iter()
        .any(|token| lowered.contains(token))
}

fn codebase_map_query_targets_engine_internals(lowered_query: &str) -> bool {
    [
        "cache",
        "cached",
        "caching",
        "evict",
        "eviction",
        "expire",
        "expiry",
        "ttl",
        "invalidation",
        "invalidate",
    ]
    .iter()
    .any(|token| lowered_query.contains(token))
}

fn codebase_map_surface_bias(node: &UniversalNode) -> f64 {
    if is_probable_codebase_map_product_surface_node(node) {
        0.45
    } else if is_probable_codebase_map_engine_internal_node(node) {
        -0.18
    } else {
        0.0
    }
}

fn is_probable_codebase_map_product_surface_node(node: &UniversalNode) -> bool {
    let path = node.location.file_path.to_ascii_lowercase();
    let symbol_name = node.name.to_ascii_lowercase();

    symbol_name.starts_with("handle_")
        || path.contains("/contextro-tools/")
        || path.contains("/contextro-server/")
        || path.contains("/tools/")
        || path.contains("/server/")
}

fn is_probable_codebase_map_engine_internal_node(node: &UniversalNode) -> bool {
    let path = node.location.file_path.to_ascii_lowercase();
    let symbol_name = node.name.to_ascii_lowercase();

    path.contains("/contextro-engines/")
        || path.contains("/engines/")
        || matches!(
            symbol_name.as_str(),
            "execute_search" | "fuse" | "adaptive_weights" | "make_result" | "search"
        )
}

fn required_codebase_map_matches(token_count: usize) -> usize {
    match token_count {
        0 => 0,
        1 => 1,
        2 => 1,
        3 => 2,
        _ => 2,
    }
}

fn tokenize_codebase_map_text(text: &str) -> Vec<String> {
    let mut normalized = String::with_capacity(text.len() * 2);
    let mut prev_was_lower_or_digit = false;

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && prev_was_lower_or_digit {
                normalized.push(' ');
            }
            normalized.push(ch.to_ascii_lowercase());
            prev_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else {
            normalized.push(' ');
            prev_was_lower_or_digit = false;
        }
    }

    let mut tokens = Vec::new();
    let mut seen = HashSet::new();

    for token in normalized.split_whitespace() {
        for variant in codebase_map_token_variants(token) {
            if seen.insert(variant.clone()) {
                tokens.push(variant);
            }
        }
    }

    tokens
}

fn codebase_map_query_targets_tests(query: &str) -> bool {
    let lowered = query.to_ascii_lowercase();
    ["test", "tests", "pytest", "spec", "fixture"]
        .iter()
        .any(|token| lowered.contains(token))
}

fn is_probable_codebase_map_test_symbol(symbol_name: &str) -> bool {
    let symbol_name = symbol_name
        .rsplit("::")
        .next()
        .unwrap_or(symbol_name)
        .rsplit('.')
        .next()
        .unwrap_or(symbol_name)
        .to_ascii_lowercase();

    symbol_name == "tests"
        || symbol_name.starts_with("test_")
        || symbol_name.ends_with("_test")
        || symbol_name.starts_with("bench_")
}

fn get_document_path_arg(args: &Value) -> Option<&str> {
    args.get("path")
        .or_else(|| args.get("file_path"))
        .and_then(|value| value.as_str())
        .filter(|path| !path.is_empty())
}

fn resolve_search_target(path: Option<&str>, codebase: Option<&str>) -> Result<PathBuf, Value> {
    if let Some(path) = path.filter(|path| !path.is_empty()) {
        return resolve_existing_path(path, codebase);
    }

    let base = codebase
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    if base.exists() {
        Ok(base.canonicalize().unwrap_or(base))
    } else {
        Err(json!({"error": format!("Path not found: {}", base.to_string_lossy())}))
    }
}

fn resolve_existing_path(path: &str, codebase: Option<&str>) -> Result<PathBuf, Value> {
    let abs_path = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        codebase
            .map(|base| Path::new(base).join(path))
            .unwrap_or_else(|| PathBuf::from(path))
    };
    if abs_path.exists() {
        Ok(abs_path.canonicalize().unwrap_or(abs_path))
    } else {
        Err(json!({"error": format!("Path not found: {}", path)}))
    }
}

fn path_matches(file_path: &str, target_path: &Path, target_is_dir: bool) -> bool {
    let normalized_file =
        std::fs::canonicalize(file_path).unwrap_or_else(|_| PathBuf::from(file_path));
    if target_is_dir {
        normalized_file == target_path || normalized_file.starts_with(target_path)
    } else {
        normalized_file == target_path
    }
}

/// Convert ast-grep-style pattern to regex.
/// $NAME -> captures a word, $$$ -> captures anything.
fn pattern_to_regex(pattern: &str) -> String {
    let escaped = regex_lite::escape(pattern);
    let triple_re = regex_lite::Regex::new(r"\\\$\\\$\\\$[A-Z_]+").unwrap();
    let result = triple_re.replace_all(&escaped, ".*").to_string();
    let result = result.replace("\\$\\$\\$", ".*");
    let re = regex_lite::Regex::new(r"\\\$[A-Z_]+").unwrap();
    re.replace_all(&result, r"[^\s(),]+").to_string()
}

fn codebase_map_token_variants(token: &str) -> Vec<String> {
    let token = token.trim().to_ascii_lowercase();
    if token.len() < 3 || is_codebase_map_stopword(&token) {
        return Vec::new();
    }

    let mut variants = vec![token.clone()];
    if let Some(stemmed) = stem_codebase_map_token(&token) {
        if stemmed != token {
            variants.push(stemmed);
        }
    }
    variants
}

fn stem_codebase_map_token(token: &str) -> Option<String> {
    let stemmed = if token.ends_with("ing") && token.len() > 5 {
        restore_codebase_map_stemmed_root(&token[..token.len() - 3])
    } else if token.ends_with("ers") && token.len() > 5 {
        token[..token.len() - 3].to_string()
    } else if token.ends_with("er") && token.len() > 4 {
        token[..token.len() - 2].to_string()
    } else if token.ends_with("ed") && token.len() > 4 {
        restore_codebase_map_stemmed_root(&token[..token.len() - 2])
    } else if token.ends_with("es") && token.len() > 4 {
        token[..token.len() - 2].to_string()
    } else if token.ends_with('s') && token.len() > 4 {
        token[..token.len() - 1].to_string()
    } else {
        token.to_string()
    };

    (stemmed.len() >= 3).then_some(stemmed)
}

fn restore_codebase_map_stemmed_root(base: &str) -> String {
    if base.ends_with("ch") || base.ends_with("sh") || base.ends_with('v') || base.ends_with('c') {
        format!("{base}e")
    } else {
        base.to_string()
    }
}

fn is_codebase_map_stopword(token: &str) -> bool {
    matches!(
        token,
        "and"
            | "are"
            | "does"
            | "for"
            | "from"
            | "how"
            | "into"
            | "the"
            | "this"
            | "that"
            | "what"
            | "when"
            | "where"
            | "which"
            | "with"
            | "work"
            | "works"
    )
}

fn resolve_edit_plan_primary_symbols(
    symbol_name: Option<&str>,
    goal: &str,
    graph: &CodeGraph,
) -> Vec<UniversalNode> {
    if let Some(symbol_name) = symbol_name.filter(|name| !name.is_empty()) {
        let matches = graph.find_nodes_by_name(symbol_name, false);
        if !matches.is_empty() {
            return matches.into_iter().take(3).collect();
        }
    }

    infer_edit_plan_symbols_from_goal(goal, graph)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditPlanCandidateMatchTier {
    ExactName,
    FuzzyName,
    Content,
}

struct EditPlanCandidateMatch {
    node: UniversalNode,
    score: f64,
    tier: EditPlanCandidateMatchTier,
}

fn infer_edit_plan_symbols_from_goal(goal: &str, graph: &CodeGraph) -> Vec<UniversalNode> {
    let all_nodes = graph.find_nodes_by_name("", false);
    let all_candidates = extract_goal_symbol_candidates(goal);
    let anchor_candidates = rank_edit_plan_goal_candidates(goal, &all_candidates);
    let mut exact_ranked_matches: HashMap<String, (UniversalNode, f64)> = HashMap::new();
    let mut fuzzy_ranked_matches: HashMap<String, (UniversalNode, f64)> = HashMap::new();
    let mut content_ranked_matches: HashMap<String, (UniversalNode, f64)> = HashMap::new();

    for (candidate, candidate_score) in anchor_candidates {
        let candidate_matches = resolve_edit_plan_candidate_matches(&candidate, graph, &all_nodes);
        for candidate_match in candidate_matches {
            let score = candidate_score + candidate_match.score;
            let bucket = match candidate_match.tier {
                EditPlanCandidateMatchTier::ExactName => &mut exact_ranked_matches,
                EditPlanCandidateMatchTier::FuzzyName => &mut fuzzy_ranked_matches,
                EditPlanCandidateMatchTier::Content => &mut content_ranked_matches,
            };
            match bucket.get_mut(&candidate_match.node.id) {
                Some(existing) if existing.1 >= score => {}
                Some(existing) => *existing = (candidate_match.node, score),
                None => {
                    bucket.insert(candidate_match.node.id.clone(), (candidate_match.node, score));
                }
            }
        }
    }

    let mut ranked_matches: Vec<(UniversalNode, f64)> = if !exact_ranked_matches.is_empty() {
        exact_ranked_matches.into_values().collect()
    } else if !fuzzy_ranked_matches.is_empty() {
        fuzzy_ranked_matches.into_values().collect()
    } else {
        content_ranked_matches.into_values().collect()
    };

    ranked_matches.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.0.location.file_path.cmp(&b.0.location.file_path))
            .then_with(|| a.0.location.start_line.cmp(&b.0.location.start_line))
    });

    ranked_matches
        .into_iter()
        .take(3)
        .map(|(node, _)| node)
        .collect()
}

fn extract_goal_symbol_candidates(goal: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    for token in goal.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':')) {
        let token = token.trim_matches(':').trim_matches('_');
        if token.len() < 3 {
            continue;
        }
        let lower = token.to_ascii_lowercase();
        if is_edit_plan_stopword(&lower) {
            continue;
        }
        if seen.insert(lower) {
            candidates.push(token.to_string());
        }
    }

    candidates
}

fn rank_edit_plan_goal_candidates(goal: &str, candidates: &[String]) -> Vec<(String, f64)> {
    let normalized_goal = goal.to_ascii_lowercase();
    let goal_tokens = tokenize_codebase_map_text(goal);
    let goal_terms: HashSet<String> = goal_tokens.iter().cloned().collect();
    let mut ranked = Vec::new();

    for (index, candidate) in candidates.iter().enumerate() {
        let candidate_lower = candidate.to_ascii_lowercase();
        let candidate_tokens = tokenize_codebase_map_text(candidate);
        let exact_phrase = normalized_goal.contains(candidate_lower.as_str());
        let symbol_like = is_edit_plan_symbol_like_candidate(candidate);
        let overlap = candidate_tokens
            .iter()
            .filter(|token| goal_terms.contains(*token))
            .count() as f64;

        let mut score = overlap * 0.25;
        if exact_phrase {
            score += 2.0;
        }
        if symbol_like {
            score += 1.3;
        }
        if candidate.chars().any(|ch| ch.is_ascii_uppercase()) {
            score += 0.45;
        }
        if candidate.contains('_') || candidate.contains(':') {
            score += 0.35;
        }
        if candidate.len() >= 8 {
            score += 0.15;
        }

        ranked.push((candidate.clone(), score - index as f64 * 0.03));
    }

    ranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| b.0.len().cmp(&a.0.len()))
            .then_with(|| a.0.cmp(&b.0))
    });
    ranked
}

fn resolve_edit_plan_candidate_matches(
    candidate: &str,
    graph: &CodeGraph,
    all_nodes: &[UniversalNode],
) -> Vec<EditPlanCandidateMatch> {
    let candidate_lower = candidate.to_ascii_lowercase();
    let candidate_tokens = tokenize_codebase_map_text(candidate);
    let mut matches: Vec<EditPlanCandidateMatch> = graph
        .find_nodes_by_name(candidate, true)
        .into_iter()
        .map(|node| {
            let score = score_edit_plan_candidate_match(&candidate_lower, &candidate_tokens, &node);
            EditPlanCandidateMatch {
                node,
                score: 3.0 + score,
                tier: EditPlanCandidateMatchTier::ExactName,
            }
        })
        .collect();

    if matches.is_empty() {
        matches = graph
            .find_nodes_by_name(candidate, false)
            .into_iter()
            .map(|node| {
                let score =
                    score_edit_plan_candidate_match(&candidate_lower, &candidate_tokens, &node);
                EditPlanCandidateMatch {
                    node,
                    score: 2.0 + score,
                    tier: EditPlanCandidateMatchTier::FuzzyName,
                }
            })
            .collect();
    }

    if matches.is_empty() {
        matches = all_nodes
            .iter()
            .filter_map(|node| {
                let score =
                    score_edit_plan_candidate_match(&candidate_lower, &candidate_tokens, node);
                (score >= 1.0).then(|| EditPlanCandidateMatch {
                    node: node.clone(),
                    score,
                    tier: EditPlanCandidateMatchTier::Content,
                })
            })
            .collect();
    }

    matches.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                edit_plan_candidate_kind_rank(&a.node).cmp(&edit_plan_candidate_kind_rank(&b.node))
            })
            .then_with(|| a.node.location.start_line.cmp(&b.node.location.start_line))
    });
    matches.truncate(4);
    matches
}

fn score_edit_plan_candidate_match(
    candidate_lower: &str,
    candidate_tokens: &[String],
    node: &UniversalNode,
) -> f64 {
    let node_name = node.name.to_ascii_lowercase();
    let qualified_name = node
        .parent
        .as_ref()
        .map(|parent| format!("{parent}.{}", node.name))
        .unwrap_or_else(|| node.name.clone())
        .to_ascii_lowercase();
    let content = node.content.to_ascii_lowercase();
    let docstring = node.docstring.as_deref().unwrap_or("").to_ascii_lowercase();
    let file_path = node.location.file_path.to_ascii_lowercase();

    let name_match_score = if node_name == candidate_lower || qualified_name == candidate_lower {
        3.0
    } else if node_name.contains(candidate_lower) || qualified_name.contains(candidate_lower) {
        2.2
    } else if candidate_lower.contains(node_name.as_str()) {
        1.4
    } else {
        0.0
    };
    let mut score = name_match_score;

    if content.contains(candidate_lower) {
        score += if node_name.contains(candidate_lower)
            || qualified_name.contains(candidate_lower)
        {
            0.75
        } else {
            0.25
        };
    }
    if docstring.contains(candidate_lower) {
        score += 0.5;
    }
    if file_path.contains(candidate_lower) {
        score += if node_name.contains(candidate_lower)
            || qualified_name.contains(candidate_lower)
        {
            0.2
        } else {
            0.05
        };
    }

    let token_overlap = candidate_tokens
        .iter()
        .filter(|token| {
            node_name.contains(token.as_str())
                || qualified_name.contains(token.as_str())
                || content.contains(token.as_str())
                || docstring.contains(token.as_str())
                || file_path.contains(token.as_str())
        })
        .count() as f64;
    let name_token_overlap = candidate_tokens
        .iter()
        .filter(|token| {
            node_name.contains(token.as_str()) || qualified_name.contains(token.as_str())
        })
        .count() as f64;
    let helper_penalty = if is_codebase_map_meta_helper_symbol(&node.name) {
        1.4
    } else if is_codebase_map_generic_helper_symbol(&node.name) {
        0.75
    } else {
        0.0
    };
    let test_penalty = if is_edit_plan_test_like_node(node)
        && !candidate_lower.contains("test")
        && !candidate_lower.contains("spec")
    {
        if name_match_score >= 2.2 {
            1.6
        } else {
            2.6
        }
    } else {
        0.0
    };
    let weak_content_only_penalty = if name_token_overlap == 0.0
        && token_overlap > 0.0
        && name_match_score == 0.0
    {
        0.75
    } else {
        0.0
    };

    score + name_token_overlap * 0.35 + token_overlap * 0.12
        - helper_penalty
        - test_penalty
        - weak_content_only_penalty
}

fn edit_plan_candidate_kind_rank(node: &UniversalNode) -> usize {
    if is_edit_plan_test_like_node(node) {
        4
    } else if is_codebase_map_meta_helper_symbol(&node.name) {
        3
    } else if is_codebase_map_generic_helper_symbol(&node.name) {
        2
    } else if node.name.chars().any(|ch| ch.is_ascii_uppercase()) {
        0
    } else {
        1
    }
}

fn is_edit_plan_symbol_like_candidate(candidate: &str) -> bool {
    candidate.contains('_')
        || candidate.contains(':')
        || candidate.chars().any(|ch| ch.is_ascii_uppercase())
}

fn is_edit_plan_test_like_node(node: &UniversalNode) -> bool {
    is_test_file(&node.location.file_path) || is_probable_codebase_map_test_symbol(&node.name)
}

fn is_edit_plan_handler_symbol(node: &UniversalNode) -> bool {
    let symbol_name = node
        .name
        .rsplit("::")
        .next()
        .unwrap_or(&node.name)
        .rsplit('.')
        .next()
        .unwrap_or(&node.name)
        .to_ascii_lowercase();

    symbol_name.starts_with("handle_")
}

fn is_edit_plan_owner_like_symbol(node: &UniversalNode) -> bool {
    node.name
        .rsplit("::")
        .next()
        .unwrap_or(&node.name)
        .rsplit('.')
        .next()
        .unwrap_or(&node.name)
        .chars()
        .any(|ch| ch.is_ascii_uppercase())
}

fn is_edit_plan_state_or_config_bridge_symbol(node: &UniversalNode) -> bool {
    let symbol_name = node
        .name
        .rsplit("::")
        .next()
        .unwrap_or(&node.name)
        .rsplit('.')
        .next()
        .unwrap_or(&node.name)
        .to_ascii_lowercase();

    symbol_name.contains("state")
        || symbol_name.contains("config")
        || symbol_name.contains("context")
        || symbol_name.contains("settings")
}

fn expand_edit_plan_bridge_symbols(
    primary_symbols: &[UniversalNode],
    goal: &str,
    graph: &CodeGraph,
    all_nodes: &[UniversalNode],
    limit: usize,
) -> Vec<UniversalNode> {
    if primary_symbols.is_empty() {
        return Vec::new();
    }

    let goal_terms = edit_plan_goal_terms(goal);
    let goal_term_set: HashSet<String> = goal_terms.iter().cloned().collect();
    let primary_ids: HashSet<String> = primary_symbols.iter().map(|node| node.id.clone()).collect();
    let primary_paths: HashSet<String> = primary_symbols
        .iter()
        .map(|node| node.location.file_path.clone())
        .collect();
    let cross_file_primary_scope = primary_paths.len() > 1;
    let mut scored: Vec<(UniversalNode, f64)> = primary_symbols
        .iter()
        .cloned()
        .map(|node| {
            let score = 3.5
                + score_edit_plan_bridge_node(
                    &node,
                    &goal_term_set,
                    &primary_paths,
                    cross_file_primary_scope,
                    false,
                );
            (node, score)
        })
        .collect();
    let mut seen: HashSet<String> = primary_ids;

    for primary in primary_symbols {
        let caller_ids: HashSet<String> = graph
            .get_callers(&primary.id)
            .into_iter()
            .map(|node| node.id)
            .collect();
        let callee_ids: HashSet<String> = graph
            .get_callees(&primary.id)
            .into_iter()
            .map(|node| node.id)
            .collect();
        let mut local_seen = HashSet::new();
        let mut neighbor_candidates: Vec<UniversalNode> = graph
            .get_callers(&primary.id)
            .into_iter()
            .chain(graph.get_callees(&primary.id).into_iter())
            .filter(|node| local_seen.insert(node.id.clone()))
            .collect();
        neighbor_candidates.extend(
            all_nodes
                .iter()
                .filter(|node| node.location.file_path == primary.location.file_path)
                .cloned(),
        );

        let mut ranked_neighbors: Vec<(UniversalNode, f64)> = neighbor_candidates
            .into_iter()
            .filter_map(|node| {
                if seen.contains(&node.id) {
                    return None;
                }
                if is_edit_plan_test_like_node(&node) {
                    return None;
                }
                let state_bridge = is_edit_plan_state_or_config_bridge_symbol(&node);
                let goal_name_overlap = edit_plan_name_overlap(&node, &goal_term_set);
                let is_caller = caller_ids.contains(&node.id);
                let is_callee = callee_ids.contains(&node.id);
                let same_file = node.location.file_path == primary.location.file_path;
                let owner_like = same_file
                    && node.location.start_line <= primary.location.start_line
                    && is_edit_plan_owner_like_symbol(&node);
                if is_callee && goal_name_overlap == 0 && !state_bridge {
                    return None;
                }
                if same_file && !is_caller && goal_name_overlap == 0 && !state_bridge && !owner_like {
                    return None;
                }
                if cross_file_primary_scope
                    && same_file
                    && !state_bridge
                    && !owner_like
                    && (is_edit_plan_handler_symbol(&node) || goal_name_overlap < 2)
                {
                    return None;
                }
                let score = score_edit_plan_bridge_node(
                    &node,
                    &goal_term_set,
                    &primary_paths,
                    cross_file_primary_scope,
                    true,
                ) + if owner_like { 0.18 } else { 0.0 };
                (score >= 0.9).then_some((node, score))
            })
            .collect();
        ranked_neighbors.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.0.location.file_path.cmp(&b.0.location.file_path))
                .then_with(|| a.0.location.start_line.cmp(&b.0.location.start_line))
        });

        for (node, score) in ranked_neighbors.into_iter().take(2) {
            if seen.insert(node.id.clone()) {
                scored.push((node, 1.2 + score));
            }
        }
    }

    let primary_concepts: HashSet<String> = primary_symbols
        .iter()
        .flat_map(edit_plan_symbol_name_tokens)
        .chain(goal_terms.iter().cloned())
        .collect();

    let mut bridge_candidates: Vec<(UniversalNode, f64)> = all_nodes
        .iter()
        .filter_map(|node| {
            if seen.contains(&node.id) {
                return None;
            }
            if is_edit_plan_test_like_node(node) {
                return None;
            }
            let score =
                score_edit_plan_bridge_concept_match(
                    node,
                    &primary_concepts,
                    &primary_paths,
                    cross_file_primary_scope,
                );
            (score >= 1.35).then(|| (node.clone(), score))
        })
        .collect();
    bridge_candidates.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.0.location.file_path.cmp(&b.0.location.file_path))
            .then_with(|| a.0.location.start_line.cmp(&b.0.location.start_line))
    });

    for (node, score) in bridge_candidates.into_iter().take(2) {
        if seen.insert(node.id.clone()) {
            scored.push((node, score));
        }
    }

    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.0.location.file_path.cmp(&b.0.location.file_path))
            .then_with(|| a.0.location.start_line.cmp(&b.0.location.start_line))
    });
    scored.truncate(limit);
    scored.into_iter().map(|(node, _)| node).collect()
}

fn score_edit_plan_bridge_node(
    node: &UniversalNode,
    goal_terms: &HashSet<String>,
    primary_paths: &HashSet<String>,
    cross_file_primary_scope: bool,
    is_secondary: bool,
) -> f64 {
    let overlap = codebase_map_concept_overlap(node, goal_terms) as f64;
    let name_overlap = edit_plan_name_overlap(node, goal_terms) as f64;
    let same_primary_file = primary_paths.contains(&node.location.file_path);
    let symbol_like = is_edit_plan_symbol_like_candidate(&node.name);
    let state_bridge = is_edit_plan_state_or_config_bridge_symbol(node);
    let owner_like = is_edit_plan_owner_like_symbol(node);
    let path = node.location.file_path.to_ascii_lowercase();
    let helper_penalty = if is_codebase_map_meta_helper_symbol(&node.name) {
        0.95
    } else if is_codebase_map_generic_helper_symbol(&node.name) && overlap < 2.0 {
        0.45
    } else {
        0.0
    };
    let state_bridge_bonus = if state_bridge && overlap >= 1.0 {
        if cross_file_primary_scope && !same_primary_file {
            0.95 + if owner_like { 0.18 } else { 0.0 }
        } else {
            0.28
        }
    } else {
        0.0
    };
    let ungrounded_penalty = if name_overlap == 0.0 && !state_bridge {
        0.40
    } else {
        0.0
    };
    let handler_penalty = if is_edit_plan_handler_symbol(node) && name_overlap == 0.0 {
        0.55
    } else {
        0.0
    };
    let cross_file_same_path_penalty = if cross_file_primary_scope
        && same_primary_file
        && !state_bridge
        && !owner_like
    {
        if is_edit_plan_handler_symbol(node) {
            1.10
        } else if name_overlap < 1.0 {
            0.70
        } else {
            0.20
        }
    } else {
        0.0
    };

    overlap * 0.35
        + name_overlap * 0.22
        + if same_primary_file {
            if cross_file_primary_scope && !state_bridge && !owner_like {
                0.08
            } else {
                0.26
            }
        } else {
            0.0
        }
        + if symbol_like { 0.15 } else { 0.0 }
        + if path.contains("/server/") || path.contains("/tools/") || path.contains("/engines/") {
            0.08
        } else {
            0.0
        }
        + if is_secondary { 0.12 } else { 0.0 }
        + state_bridge_bonus
        - helper_penalty
        - ungrounded_penalty
        - handler_penalty
        - cross_file_same_path_penalty
}

fn score_edit_plan_bridge_concept_match(
    node: &UniversalNode,
    primary_concepts: &HashSet<String>,
    primary_paths: &HashSet<String>,
    cross_file_primary_scope: bool,
) -> f64 {
    let concept_overlap = codebase_map_concept_overlap(node, primary_concepts) as f64;
    let symbol_overlap = codebase_map_symbol_concept_overlap(node, primary_concepts) as f64;
    let name_overlap = edit_plan_name_overlap(node, primary_concepts) as f64;
    let same_path = primary_paths.contains(&node.location.file_path);
    let state_bridge = is_edit_plan_state_or_config_bridge_symbol(node);
    let owner_like = is_edit_plan_owner_like_symbol(node);
    if cross_file_primary_scope && same_path && !state_bridge && !owner_like && name_overlap < 2.0 {
        return 0.0;
    }
    if name_overlap == 0.0 && !state_bridge {
        return 0.0;
    }
    let helper_penalty = if is_codebase_map_meta_helper_symbol(&node.name) {
        1.1
    } else if is_codebase_map_generic_helper_symbol(&node.name) && name_overlap < 2.0 {
        0.55
    } else {
        0.0
    };
    let state_bridge_bonus = if state_bridge && (concept_overlap >= 2.0 || name_overlap >= 1.0) {
        if cross_file_primary_scope && !same_path {
            1.10 + if owner_like { 0.18 } else { 0.0 }
        } else {
            0.72
        }
    } else {
        0.0
    };
    let handler_penalty = if is_edit_plan_handler_symbol(node) && name_overlap < 1.0 {
        0.40
    } else {
        0.0
    };
    let cross_file_same_path_penalty = if cross_file_primary_scope && same_path && !state_bridge && !owner_like {
        if is_edit_plan_handler_symbol(node) {
            0.90
        } else if name_overlap < 2.0 {
            0.60
        } else {
            0.20
        }
    } else {
        0.0
    };

    concept_overlap * 0.30
        + symbol_overlap * 0.12
        + name_overlap * 0.28
        + if same_path {
            if cross_file_primary_scope && !state_bridge && !owner_like {
                0.04
            } else {
                0.16
            }
        } else {
            0.0
        }
        + if is_edit_plan_symbol_like_candidate(&node.name) {
            0.10
        } else {
            0.0
        }
        + state_bridge_bonus
        - helper_penalty
        - handler_penalty
        - cross_file_same_path_penalty
}

fn is_edit_plan_stopword(token: &str) -> bool {
    matches!(
        token,
        "add"
            | "change"
            | "function"
            | "extract"
            | "file"
            | "goal"
            | "into"
            | "move"
            | "per"
            | "plan"
            | "refactor"
            | "rename"
            | "replace"
            | "result"
            | "separate"
            | "the"
            | "this"
            | "tool"
            | "update"
            | "using"
    )
}

fn edit_plan_goal_terms(goal: &str) -> Vec<String> {
    tokenize_codebase_map_text(goal)
        .into_iter()
        .filter(|token| !is_edit_plan_stopword(token))
        .collect()
}

fn edit_plan_symbol_name_tokens(node: &UniversalNode) -> HashSet<String> {
    let qualified_name = node
        .parent
        .as_ref()
        .map(|parent| format!("{parent}.{}", node.name))
        .unwrap_or_else(|| node.name.clone());
    [node.name.as_str(), qualified_name.as_str()]
        .into_iter()
        .flat_map(tokenize_codebase_map_text)
        .collect()
}

fn edit_plan_name_overlap(node: &UniversalNode, concept_tokens: &HashSet<String>) -> usize {
    let name_tokens = edit_plan_symbol_name_tokens(node);
    concept_tokens
        .iter()
        .filter(|term| {
            name_tokens.iter().any(|candidate| {
                candidate == *term
                    || candidate.contains(term.as_str())
                    || term.contains(candidate.as_str())
            })
        })
        .count()
}

fn add_edit_plan_symbol(
    affected_symbols: &mut Vec<Value>,
    seen_symbol_ids: &mut HashSet<String>,
    graph: &CodeGraph,
    node: &UniversalNode,
    codebase: Option<&str>,
    role: &str,
) {
    if !seen_symbol_ids.insert(node.id.clone()) {
        return;
    }
    let (callers, callees) = graph.get_node_degree(&node.id);
    affected_symbols.push(json!({
        "name": node.name,
        "file": strip_base(&node.location.file_path, codebase),
        "line": node.location.start_line,
        "callers": callers,
        "callees": callees,
        "role": role,
    }));
}

fn add_edit_plan_neighbors(
    affected_symbols: &mut Vec<Value>,
    seen_symbol_ids: &mut HashSet<String>,
    target_files: &mut Vec<String>,
    risks: &mut Vec<String>,
    graph: &CodeGraph,
    node: &UniversalNode,
    goal_terms: &HashSet<String>,
    codebase: Option<&str>,
) {
    let anchor_terms = edit_plan_symbol_name_tokens(node);
    let (anchor_callers, anchor_callees) = graph.get_node_degree(&node.id);
    let high_degree_anchor =
        anchor_callers >= 2 || anchor_callees >= 5 || anchor_callers + anchor_callees >= 6;
    let mut seen_neighbors = HashSet::new();
    let mut neighbor_candidates: Vec<UniversalNode> = graph
        .get_callers(&node.id)
        .into_iter()
        .chain(graph.get_callees(&node.id).into_iter())
        .filter(|neighbor| seen_neighbors.insert(neighbor.id.clone()))
        .collect();
    neighbor_candidates.extend(
        graph.find_nodes_by_name("", false).into_iter().filter(|candidate| {
            candidate.location.file_path == node.location.file_path
                && candidate.location.start_line <= node.location.start_line
                && candidate.name.chars().any(|ch| ch.is_ascii_uppercase())
                && seen_neighbors.insert(candidate.id.clone())
        }),
    );
    let mut neighbors: Vec<(UniversalNode, f64)> = neighbor_candidates
        .into_iter()
        .filter_map(|neighbor| {
            if is_edit_plan_test_like_node(&neighbor) {
                return None;
            }

            let goal_overlap = codebase_map_symbol_concept_overlap(&neighbor, goal_terms) as f64;
            let anchor_overlap = codebase_map_symbol_concept_overlap(&neighbor, &anchor_terms) as f64;
            let goal_name_overlap = edit_plan_name_overlap(&neighbor, goal_terms) as f64;
            let anchor_name_overlap = edit_plan_name_overlap(&neighbor, &anchor_terms) as f64;
            let conceptual_overlap = goal_overlap + anchor_overlap;
            let name_overlap = goal_name_overlap + anchor_name_overlap;
            let same_file = neighbor.location.file_path == node.location.file_path;
            let state_bridge = is_edit_plan_state_or_config_bridge_symbol(&neighbor);
            let owner_like = same_file
                && neighbor.location.start_line <= node.location.start_line
                && neighbor.name.chars().any(|ch| ch.is_ascii_uppercase());
            let handler_like = is_edit_plan_handler_symbol(&neighbor);
            if high_degree_anchor && !state_bridge && name_overlap < 1.0 {
                return None;
            }
            if high_degree_anchor && !state_bridge && goal_name_overlap == 0.0 && anchor_name_overlap == 0.0 {
                return None;
            }
            if high_degree_anchor && same_file && handler_like && goal_name_overlap == 0.0 {
                return None;
            }
            if high_degree_anchor && same_file && !owner_like && !state_bridge && name_overlap < 2.0 {
                return None;
            }
            let helper_penalty = if is_codebase_map_meta_helper_symbol(&neighbor.name) {
                0.95
            } else if is_codebase_map_generic_helper_symbol(&neighbor.name)
                && conceptual_overlap < 2.0
            {
                0.40
            } else {
                0.0
            };
            let handler_penalty = if handler_like && goal_name_overlap == 0.0 {
                0.40
            } else {
                0.0
            };
            let state_bridge_bonus = if state_bridge && (goal_overlap >= 1.0 || anchor_overlap >= 1.0)
            {
                0.25
            } else {
                0.0
            };

            let score = goal_overlap * 0.28
                + anchor_overlap * 0.24
                + goal_name_overlap * 0.22
                + anchor_name_overlap * 0.18
                + if same_file { 0.35 } else { 0.10 }
                + 0.18
                + state_bridge_bonus
                - helper_penalty
                - handler_penalty;

            let min_score = if high_degree_anchor { 0.90 } else { 0.45 };
            (score >= min_score).then_some((neighbor, score))
        })
        .collect();
    neighbors.sort_by(|a, b| {
        let a_same_file = a.0.location.file_path == node.location.file_path;
        let b_same_file = b.0.location.file_path == node.location.file_path;
        b.1.partial_cmp(&a.1)
            .unwrap_or(Ordering::Equal)
            .then_with(|| b_same_file.cmp(&a_same_file))
            .then_with(|| a.0.location.start_line.cmp(&b.0.location.start_line))
    });

    for (neighbor, _) in neighbors
        .into_iter()
        .take(if high_degree_anchor { 2 } else { 3 })
    {
        let file = strip_base(&neighbor.location.file_path, codebase);
        if !target_files.contains(&file) {
            target_files.push(file.clone());
        }
        add_edit_plan_symbol(
            affected_symbols,
            seen_symbol_ids,
            graph,
            &neighbor,
            codebase,
            "neighbor",
        );

        let (callers, _) = graph.get_node_degree(&neighbor.id);
        if callers > 5 {
            risks.push(format!(
                "{} has {} callers — high blast radius",
                neighbor.name, callers
            ));
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use contextro_core::graph::{
        RelationshipType, UniversalLocation, UniversalNode, UniversalRelationship,
    };
    use contextro_core::NodeType;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("contextro-code-{unique}-{name}"));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn test_node(
        id: &str,
        name: &str,
        file_path: &str,
        start_line: u32,
        content: &str,
    ) -> UniversalNode {
        UniversalNode {
            id: id.into(),
            name: name.into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: file_path.into(),
                start_line,
                end_line: start_line + 1,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            content: content.into(),
            language: "rust".into(),
            ..Default::default()
        }
    }

    fn add_call(graph: &CodeGraph, source_id: &str, target_id: &str) {
        graph.add_relationship(UniversalRelationship {
            id: format!("{source_id}->{target_id}"),
            source_id: source_id.into(),
            target_id: target_id.into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });
    }

    #[test]
    fn test_get_document_symbols_accepts_path_alias() {
        let dir = temp_dir("symbols");
        let file = dir.join("main.py");
        std::fs::write(&file, "def hello():\n    return 1\n").unwrap();

        let result = get_document_symbols(
            &json!({"path": file.to_string_lossy().to_string()}),
            Some(dir.to_string_lossy().as_ref()),
        );
        assert_eq!(result["total"], 1);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_search_codebase_map_errors_on_missing_path() {
        let graph = CodeGraph::new();
        let result = search_codebase_map(&json!({"path":"missing"}), &graph, None);
        assert!(result.get("error").is_some());
    }

    #[test]
    fn test_search_codebase_map_handles_absolute_path_filter() {
        let dir = temp_dir("map");
        let file = dir.join("lib.rs");
        std::fs::write(&file, "fn alpha() {}\n").unwrap();

        let graph = CodeGraph::new();
        graph.add_node(UniversalNode {
            id: "alpha".into(),
            name: "alpha".into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: file.to_string_lossy().to_string(),
                start_line: 1,
                end_line: 1,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        });

        let result = search_codebase_map(
            &json!({"path": dir.to_string_lossy().to_string()}),
            &graph,
            Some(dir.to_string_lossy().as_ref()),
        );
        assert_eq!(result["total_files"], 1);
        assert_eq!(result["total_symbols"], 1);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_search_codebase_map_matches_natural_language_queries() {
        let graph = CodeGraph::new();
        graph.add_node(UniversalNode {
            id: "repo-add".into(),
            name: "handle_repo_add".into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: "/tmp/contextro/crates/contextro-tools/src/git_tools.rs".into(),
                start_line: 10,
                end_line: 20,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            content: "pub fn handle_repo_add(path: &str) { registry.add(path); }".into(),
            language: "rust".into(),
            ..Default::default()
        });
        graph.add_node(UniversalNode {
            id: "repo-registry-add".into(),
            name: "add".into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: "/tmp/contextro/crates/contextro-tools/src/git_tools.rs".into(),
                start_line: 30,
                end_line: 45,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            content: "pub fn add(&self, path: &str) { self.persist_repos(); }".into(),
            language: "rust".into(),
            parent: Some("RepoRegistry".into()),
            ..Default::default()
        });

        let result = search_codebase_map(
            &json!({"query":"repo add persistence"}),
            &graph,
            Some("/tmp/contextro"),
        );

        assert_eq!(result["total_files"], 1);
        assert!(result["total_symbols"].as_u64().unwrap_or(0) >= 1);
        let names: Vec<&str> = result["files"][0]["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|symbol| symbol["name"].as_str())
            .collect();
        assert!(
            names.contains(&"handle_repo_add") || names.contains(&"add"),
            "unexpected matches: {:?}",
            names
        );
    }

    #[test]
    fn test_pattern_search_errors_on_missing_path() {
        let result = pattern_search(
            &json!({"pattern":"truncate_chars","path":"does/not/exist.rs"}),
            None,
        );

        assert_eq!(result["error"], "Path not found: does/not/exist.rs");
    }

    #[test]
    fn test_pattern_rewrite_errors_on_missing_path() {
        let result = pattern_rewrite(
            &json!({
                "pattern":"truncate_chars",
                "replacement":"truncate_text",
                "path":"does/not/exist.rs",
                "dry_run":true
            }),
            None,
        );

        assert_eq!(result["error"], "Path not found: does/not/exist.rs");
    }

    #[test]
    fn test_pattern_search_supports_metavariables() {
        let dir = temp_dir("pattern-search-metavars");
        let file = dir.join("lib.rs");
        std::fs::write(&file, "fn truncate_chars(text: &str, max_chars: usize) -> String {\n    text.to_string()\n}\n").unwrap();

        let result = pattern_search(
            &json!({
                "pattern":"fn $NAME($$$ARGS)",
                "language":"rust",
                "path": file.to_string_lossy().to_string()
            }),
            None,
        );

        assert_eq!(result["total"], 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_pattern_search_accepts_repo_relative_directory_path() {
        let dir = temp_dir("pattern-search-relative-dir");
        let nested = dir.join("src");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            nested.join("lib.rs"),
            "fn handle_alpha() {}\nfn beta() {}\n",
        )
        .unwrap();

        let result = pattern_search(
            &json!({
                "pattern":"fn handle_",
                "path":"src",
                "language":"rust"
            }),
            Some(dir.to_string_lossy().as_ref()),
        );

        assert_eq!(result["total"], 1);
        assert_eq!(result["matches"][0]["file"], "src/lib.rs");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_pattern_rewrite_accepts_repo_relative_file_path() {
        let dir = temp_dir("pattern-rewrite-relative-file");
        let nested = dir.join("src");
        std::fs::create_dir_all(&nested).unwrap();
        let file = nested.join("lib.rs");
        std::fs::write(&file, "fn handle_search() {}\n").unwrap();

        let result = pattern_rewrite(
            &json!({
                "pattern":"handle_search",
                "replacement":"handle_lookup",
                "path":"src/lib.rs",
                "dry_run":true
            }),
            Some(dir.to_string_lossy().as_ref()),
        );

        assert_eq!(result["total_files"], 1);
        assert_eq!(result["changes"][0]["file"], "src/lib.rs");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_lookup_symbols_rejects_empty_array_explicitly() {
        let graph = CodeGraph::new();
        let result = lookup_symbols(&json!({"symbols":[]}), &graph, None);

        assert_eq!(
            result["error"],
            "Parameter 'symbols' must contain at least one symbol name."
        );
    }

    #[test]
    fn test_search_symbols_missing_input_mentions_query_alias() {
        let graph = CodeGraph::new();
        let result = search_symbols(&json!({}), &graph, None);

        assert!(result["hint"].as_str().unwrap_or("").contains("query"));
    }

    #[test]
    fn test_edit_plan_only_returns_exact_related_test_matches() {
        let graph = CodeGraph::new();
        graph.add_node(UniversalNode {
            id: "truncate-chars".into(),
            name: "truncate_chars".into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: "/tmp/contextro/crates/contextro-tools/src/code.rs".into(),
                start_line: 14,
                end_line: 21,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        });

        let result = edit_plan(
            &json!({"goal":"rename truncate_chars to truncate_text","symbol_name":"truncate_chars"}),
            &graph,
            Some("/tmp/contextro"),
        );

        assert_eq!(result["related_tests"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_edit_plan_infers_scope_from_goal_only() {
        let graph = CodeGraph::new();
        let file = "/tmp/contextro/crates/contextro-tools/src/search.rs";
        graph.add_node(test_node(
            "handle-search",
            "handle_search",
            file,
            17,
            "pub fn handle_search() { rerank_natural_language_results(); drop_low_confidence_noise(); }",
        ));
        graph.add_node(test_node(
            "rerank",
            "rerank_natural_language_results",
            file,
            21,
            "fn rerank_natural_language_results() {}",
        ));
        graph.add_node(test_node(
            "drop-noise",
            "drop_low_confidence_noise",
            file,
            130,
            "fn drop_low_confidence_noise() {}",
        ));
        add_call(&graph, "handle-search", "rerank");
        add_call(&graph, "handle-search", "drop-noise");

        let result = edit_plan(
            &json!({"goal":"refactor handle_search to extract reranking into a separate function"}),
            &graph,
            Some("/tmp/contextro"),
        );

        let affected = result["affected_symbols"].as_array().unwrap();
        let names: Vec<&str> = affected
            .iter()
            .filter_map(|symbol| symbol["name"].as_str())
            .collect();

        assert_eq!(result["confidence"], "high");
        assert_eq!(
            result["target_files"][0],
            "crates/contextro-tools/src/search.rs"
        );
        assert!(names.contains(&"handle_search"));
        assert!(names.contains(&"rerank_natural_language_results"));
        assert!(names.contains(&"drop_low_confidence_noise"));
        assert!(result["next_steps"]
            .as_array()
            .unwrap()
            .iter()
            .all(|step| step.as_str() != Some("Review the diff preview before applying")));
    }

    #[test]
    fn test_edit_plan_reports_low_confidence_for_empty_scope() {
        let graph = CodeGraph::new();
        let result = edit_plan(
            &json!({"goal":"refactor impossible_missing_symbol to extract helper"}),
            &graph,
            Some("/tmp/contextro"),
        );

        assert_eq!(result["confidence"], "low");
        assert_eq!(result["affected_symbols"].as_array().unwrap().len(), 0);
        assert_eq!(result["target_files"].as_array().unwrap().len(), 0);
        assert!(result["next_steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step.as_str() == Some("Resolve the target symbol or file before editing")));
    }

    #[test]
    fn test_edit_plan_prioritizes_cross_subsystem_anchors_before_generic_goal_tokens() {
        let graph = CodeGraph::new();
        let server_file = "/tmp/contextro/crates/contextro-server/src/main.rs";
        let state_file = "/tmp/contextro/crates/contextro-server/src/state.rs";
        let cache_file = "/tmp/contextro/crates/contextro-engines/src/cache.rs";
        let unrelated_file = "/tmp/contextro/crates/contextro-tools/src/code.rs";

        graph.add_node(test_node(
            "dispatch",
            "dispatch",
            server_file,
            50,
            "fn dispatch(&self, name: &str, args: Value) { self.state.query_cache.get(name); }",
        ));
        graph.add_node(test_node(
            "app-state",
            "AppState",
            state_file,
            27,
            "pub struct AppState { pub query_cache: Arc<QueryCache>, pub graph: Arc<CodeGraph> }",
        ));
        graph.add_node(test_node(
            "query-cache",
            "QueryCache",
            cache_file,
            14,
            "pub struct QueryCache { entries: DashMap<String, CacheEntry> }",
        ));
        graph.add_node(test_node(
            "generic-add",
            "add_edit_plan_symbol",
            unrelated_file,
            2166,
            "fn add_edit_plan_symbol() {}",
        ));
        graph.add_node(test_node(
            "generic-result",
            "accumulate_result",
            unrelated_file,
            1400,
            "fn accumulate_result() { result.push(value); }",
        ));

        let result = edit_plan(
            &json!({"goal":"add per-tool result caching to the dispatch function using QueryCache"}),
            &graph,
            Some("/tmp/contextro"),
        );

        let affected = result["affected_symbols"].as_array().unwrap();
        let names: Vec<&str> = affected
            .iter()
            .filter_map(|symbol| symbol["name"].as_str())
            .collect();
        let primary_names: Vec<&str> = affected
            .iter()
            .filter(|symbol| symbol["role"].as_str() == Some("primary"))
            .filter_map(|symbol| symbol["name"].as_str())
            .collect();
        let target_files: Vec<&str> = result["target_files"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str())
            .collect();

        assert_eq!(result["confidence"], "high");
        assert!(primary_names.contains(&"dispatch"));
        assert!(primary_names.contains(&"QueryCache"));
        assert!(names.contains(&"AppState"));
        assert!(!primary_names.contains(&"add_edit_plan_symbol"));
        assert!(!primary_names.contains(&"accumulate_result"));
        assert!(target_files.contains(&"crates/contextro-server/src/main.rs"));
        assert!(target_files.contains(&"crates/contextro-server/src/state.rs"));
        assert!(target_files.contains(&"crates/contextro-engines/src/cache.rs"));
    }

    #[test]
    fn test_edit_plan_bridge_expansion_adds_same_file_and_conceptual_bridge_symbols() {
        let graph = CodeGraph::new();
        let server_file = "/tmp/contextro/crates/contextro-server/src/main.rs";
        let state_file = "/tmp/contextro/crates/contextro-server/src/state.rs";
        let cache_file = "/tmp/contextro/crates/contextro-engines/src/cache.rs";

        graph.add_node(test_node(
            "dispatch",
            "dispatch",
            server_file,
            50,
            "fn dispatch(&self, name: &str, args: Value) { let s = &self.state; }",
        ));
        graph.add_node(test_node(
            "contextro-server",
            "ContextroServer",
            server_file,
            21,
            "pub struct ContextroServer { state: Arc<AppState> }",
        ));
        graph.add_node(test_node(
            "app-state",
            "AppState",
            state_file,
            27,
            "pub struct AppState { pub query_cache: Arc<QueryCache> }",
        ));
        graph.add_node(test_node(
            "query-cache-field",
            "query_cache",
            state_file,
            32,
            "pub query_cache: Arc<QueryCache>",
        ));
        graph.add_node(test_node(
            "query-cache",
            "QueryCache",
            cache_file,
            14,
            "pub struct QueryCache { entries: DashMap<String, CacheEntry> }",
        ));

        let result = edit_plan(
            &json!({"goal":"add per-tool result caching to the dispatch function using QueryCache"}),
            &graph,
            Some("/tmp/contextro"),
        );

        let affected = result["affected_symbols"].as_array().unwrap();
        let names: Vec<&str> = affected
            .iter()
            .filter_map(|symbol| symbol["name"].as_str())
            .collect();

        assert!(names.contains(&"dispatch"));
        assert!(names.contains(&"ContextroServer"));
        assert!(names.contains(&"AppState"));
        assert!(names.contains(&"query_cache") || names.contains(&"QueryCache"));
    }

    #[test]
    fn test_edit_plan_prefers_state_bridge_symbols_and_filters_test_like_distractors() {
        let graph = CodeGraph::new();
        let server_file = "/tmp/contextro/crates/contextro-server/src/main.rs";
        let state_file = "/tmp/contextro/crates/contextro-server/src/state.rs";
        let cache_file = "/tmp/contextro/crates/contextro-engines/src/cache.rs";
        let tool_file = "/tmp/contextro/crates/contextro-tools/src/code.rs";
        let test_file = "/tmp/contextro/crates/contextro-server/tests/dispatch.rs";

        graph.add_node(test_node(
            "dispatch",
            "dispatch",
            server_file,
            50,
            "fn dispatch(&self, name: &str, args: Value) { self.state.query_cache.get(name); }",
        ));
        graph.add_node(test_node(
            "server",
            "ContextroServer",
            server_file,
            21,
            "pub struct ContextroServer { state: Arc<AppState> }",
        ));
        graph.add_node(test_node(
            "app-state",
            "AppState",
            state_file,
            27,
            "pub struct AppState { pub query_cache: Arc<QueryCache>, pub graph: Arc<CodeGraph> }",
        ));
        graph.add_node(test_node(
            "query-cache-field",
            "query_cache",
            state_file,
            32,
            "pub query_cache: Arc<QueryCache>",
        ));
        graph.add_node(test_node(
            "query-cache",
            "QueryCache",
            cache_file,
            14,
            "pub struct QueryCache { entries: DashMap<String, CacheEntry> }",
        ));
        graph.add_node(test_node(
            "helper-add",
            "add_edit_plan_symbol",
            tool_file,
            2544,
            "fn add_edit_plan_symbol() { affected_symbols.push(value); }",
        ));
        graph.add_node(test_node(
            "helper-accumulate",
            "accumulate_result",
            tool_file,
            1404,
            "fn accumulate_result() { result.push(value); }",
        ));
        graph.add_node(test_node(
            "dispatch-test",
            "test_dispatch_query_cache",
            test_file,
            10,
            "fn test_dispatch_query_cache() { assert!(true); }",
        ));

        add_call(&graph, "server", "dispatch");
        add_call(&graph, "dispatch", "app-state");
        add_call(&graph, "app-state", "query-cache");
        add_call(&graph, "app-state", "query-cache-field");
        add_call(&graph, "dispatch-test", "dispatch");
        add_call(&graph, "helper-accumulate", "dispatch");

        let result = edit_plan(
            &json!({"goal":"add per-tool result caching to the dispatch function using QueryCache"}),
            &graph,
            Some("/tmp/contextro"),
        );

        let affected = result["affected_symbols"].as_array().unwrap();
        let names: Vec<&str> = affected
            .iter()
            .filter_map(|symbol| symbol["name"].as_str())
            .collect();

        assert!(names.contains(&"dispatch"), "unexpected names: {:?}", names);
        assert!(names.contains(&"QueryCache"), "unexpected names: {:?}", names);
        assert!(names.contains(&"AppState"), "unexpected names: {:?}", names);
        assert!(
            !names.contains(&"test_dispatch_query_cache"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            !names.contains(&"add_edit_plan_symbol"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            !names.contains(&"accumulate_result"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            affected.len() <= 5,
            "too many affected symbols returned: {:?}",
            names
        );
    }

    #[test]
    fn test_edit_plan_real_runtime_query_stays_focused_on_dispatch_cache_path() {
        let graph = CodeGraph::new();
        let main_file = "/tmp/contextro/crates/contextro-server/src/main.rs";
        let state_file = "/tmp/contextro/crates/contextro-server/src/state.rs";
        let cache_file = "/tmp/contextro/crates/contextro-engines/src/cache.rs";
        let bm25_file = "/tmp/contextro/crates/contextro-engines/src/bm25.rs";
        let chunk_file = "/tmp/contextro/crates/contextro-tools/src/search.rs";
        let analysis_file = "/tmp/contextro/crates/contextro-tools/src/analysis.rs";

        graph.add_node(test_node(
            "dispatch",
            "dispatch",
            main_file,
            50,
            "fn dispatch(&self, name: &str, args: Value) { self.state.query_cache.get(name); self.state.query_cache.insert(name, args); }",
        ));
        graph.add_node(test_node(
            "server",
            "ContextroServer",
            main_file,
            21,
            "pub struct ContextroServer { state: Arc<AppState> }",
        ));
        graph.add_node(test_node(
            "app-state",
            "AppState",
            state_file,
            27,
            "pub struct AppState { pub query_cache: Arc<QueryCache>, pub graph: Arc<CodeGraph> }",
        ));
        graph.add_node(test_node(
            "query-cache",
            "QueryCache",
            cache_file,
            14,
            "pub struct QueryCache { entries: DashMap<String, CacheEntry> }",
        ));
        graph.add_node(test_node(
            "dead-code",
            "handle_dead_code",
            analysis_file,
            322,
            "pub fn handle_dead_code() { /* unrelated analysis handler */ }",
        ));
        graph.add_node(test_node(
            "circular",
            "handle_circular_dependencies",
            analysis_file,
            429,
            "pub fn handle_circular_dependencies() { /* unrelated dependency handler */ }",
        ));
        graph.add_node(test_node(
            "search",
            "search",
            main_file,
            110,
            "fn search() { /* dispatches search tools */ }",
        ));
        graph.add_node(test_node(
            "make-chunk",
            "make_chunk",
            chunk_file,
            696,
            "fn make_chunk(id: &str, text: &str) -> CodeChunk { /* chunk builder */ }",
        ));
        graph.add_node(test_node(
            "bm25-cache-test",
            "test_bm25_search_recovers_cache_from_caching_query",
            bm25_file,
            389,
            "fn test_bm25_search_recovers_cache_from_caching_query() { assert!(cache_hit); }",
        ));

        add_call(&graph, "server", "dispatch");
        add_call(&graph, "dispatch", "app-state");
        add_call(&graph, "app-state", "query-cache");
        add_call(&graph, "dispatch", "search");
        add_call(&graph, "dispatch", "dead-code");
        add_call(&graph, "dispatch", "circular");
        add_call(&graph, "dispatch", "make-chunk");

        let result = edit_plan(
            &json!({"goal":"add per-tool result caching to the dispatch function using QueryCache"}),
            &graph,
            Some("/tmp/contextro"),
        );

        let affected = result["affected_symbols"].as_array().unwrap();
        let names: Vec<&str> = affected
            .iter()
            .filter_map(|symbol| symbol["name"].as_str())
            .collect();
        let primary_names: Vec<&str> = affected
            .iter()
            .filter(|symbol| symbol["role"].as_str() == Some("primary"))
            .filter_map(|symbol| symbol["name"].as_str())
            .collect();
        let target_files: Vec<&str> = result["target_files"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str())
            .collect();

        assert!(primary_names.contains(&"dispatch"), "unexpected primaries: {:?}", primary_names);
        assert!(primary_names.contains(&"QueryCache"), "unexpected primaries: {:?}", primary_names);
        assert!(names.contains(&"AppState"), "unexpected names: {:?}", names);
        assert!(!names.contains(&"test_bm25_search_recovers_cache_from_caching_query"), "unexpected names: {:?}", names);
        assert!(!names.contains(&"handle_circular_dependencies"), "unexpected names: {:?}", names);
        assert!(!names.contains(&"handle_dead_code"), "unexpected names: {:?}", names);
        assert!(!names.contains(&"search"), "unexpected names: {:?}", names);
        assert!(!names.contains(&"make_chunk"), "unexpected names: {:?}", names);
        assert!(target_files.contains(&"crates/contextro-server/src/main.rs"));
        assert!(target_files.contains(&"crates/contextro-server/src/state.rs"));
        assert!(target_files.contains(&"crates/contextro-engines/src/cache.rs"));
        assert_eq!(target_files.len(), 3, "unexpected target files: {:?}", target_files);
        assert!(affected.len() <= 5, "unexpected affected symbols: {:?}", names);
    }

    #[test]
    fn test_edit_plan_regression_excludes_rc_dispatch_cache_contamination() {
        let graph = CodeGraph::new();
        let code_file = "/tmp/contextro/crates/contextro-tools/src/code.rs";
        let analysis_file = "/tmp/contextro/crates/contextro-tools/src/analysis.rs";

        graph.add_node(test_node(
            "dispatch",
            "dispatch",
            code_file,
            50,
            "fn dispatch(&self, operation: &str, args: Value) { self.state.query_cache.get(operation); self.state.query_cache.insert(operation, CacheEntry::new()); }",
        ));
        graph.add_node(test_node(
            "query-cache",
            "QueryCache",
            code_file,
            14,
            "pub struct QueryCache { entries: DashMap<String, CacheEntry> }",
        ));
        graph.add_node(test_node(
            "cache-entry",
            "CacheEntry",
            code_file,
            19,
            "pub struct CacheEntry { value: Value, hit_rate: usize }",
        ));
        graph.add_node(test_node(
            "server",
            "ContextroServer",
            code_file,
            5,
            "pub struct ContextroServer { state: Arc<AppState> }",
        ));
        graph.add_node(test_node(
            "app-state",
            "AppState",
            code_file,
            9,
            "pub struct AppState { pub query_cache: Arc<QueryCache>, pub graph: Arc<CodeGraph> }",
        ));
        graph.add_node(test_node(
            "handle-index",
            "handle_index",
            code_file,
            80,
            "fn handle_index(state: &AppState) { dispatch(); }",
        ));
        graph.add_node(test_node(
            "handle-focus",
            "handle_focus",
            code_file,
            90,
            "fn handle_focus(state: &AppState) { dispatch(); }",
        ));
        graph.add_node(test_node(
            "handle-status",
            "handle_status",
            code_file,
            100,
            "fn handle_status(state: &AppState) { dispatch(); }",
        ));
        graph.add_node(test_node(
            "clear-scope",
            "clear_active_scope",
            code_file,
            110,
            "fn clear_active_scope(state: &AppState) { state.query_cache.clear(); }",
        ));
        graph.add_node(test_node(
            "hit-rate",
            "hit_rate",
            code_file,
            120,
            "fn hit_rate(entry: &CacheEntry) -> f64 { entry.hit_rate as f64 }",
        ));
        graph.add_node(test_node(
            "analysis-plan",
            "build_analysis_plan",
            analysis_file,
            40,
            "fn build_analysis_plan() { handle_focus(); handle_status(); }",
        ));

        add_call(&graph, "server", "dispatch");
        add_call(&graph, "dispatch", "app-state");
        add_call(&graph, "app-state", "query-cache");
        add_call(&graph, "query-cache", "cache-entry");
        add_call(&graph, "dispatch", "handle-index");
        add_call(&graph, "dispatch", "handle-focus");
        add_call(&graph, "dispatch", "handle-status");
        add_call(&graph, "dispatch", "clear-scope");
        add_call(&graph, "dispatch", "hit-rate");
        add_call(&graph, "analysis-plan", "handle-focus");
        add_call(&graph, "analysis-plan", "handle-status");

        let result = edit_plan(
            &json!({"goal":"add per-tool result caching to the dispatch function using QueryCache"}),
            &graph,
            Some("/tmp/contextro"),
        );

        let affected = result["affected_symbols"].as_array().unwrap();
        let names: Vec<&str> = affected
            .iter()
            .filter_map(|symbol| symbol["name"].as_str())
            .collect();
        let target_files: Vec<&str> = result["target_files"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str())
            .collect();

        assert!(names.contains(&"dispatch"), "unexpected names: {:?}", names);
        assert!(names.contains(&"QueryCache"), "unexpected names: {:?}", names);
        assert!(names.contains(&"AppState"), "unexpected names: {:?}", names);
        assert!(!names.contains(&"handle_focus"), "unexpected names: {:?}", names);
        assert!(!names.contains(&"handle_status"), "unexpected names: {:?}", names);
        assert!(!names.contains(&"clear_active_scope"), "unexpected names: {:?}", names);
        assert!(!target_files.contains(&"crates/contextro-tools/src/analysis.rs"), "unexpected target files: {:?}", target_files);
        assert_eq!(target_files, vec!["crates/contextro-tools/src/code.rs"]);
    }

    #[test]
    fn test_edit_plan_regression_promotes_app_state_and_excludes_leaky_main_bridges() {
        let graph = CodeGraph::new();
        let main_file = "/tmp/contextro/crates/contextro-server/src/main.rs";
        let state_file = "/tmp/contextro/crates/contextro-server/src/state.rs";
        let cache_file = "/tmp/contextro/crates/contextro-engines/src/cache.rs";

        graph.add_node(test_node(
            "dispatch",
            "dispatch",
            main_file,
            90,
            "fn dispatch(&self, operation: &str, args: Value) { self.state.query_cache.get(operation); call_tool(operation, args); }",
        ));
        graph.add_node(test_node(
            "call-tool",
            "call_tool",
            main_file,
            140,
            "fn call_tool(operation: &str, args: Value) -> Value { dispatch_tool(operation, args) }",
        ));
        graph.add_node(test_node(
            "server",
            "ContextroServer",
            main_file,
            20,
            "pub struct ContextroServer { state: Arc<AppState> }",
        ));
        graph.add_node(test_node(
            "handle-status",
            "handle_status",
            main_file,
            200,
            "fn handle_status(state: &AppState) -> Value { state.query_cache.hit_rate(); json!({}) }",
        ));
        graph.add_node(test_node(
            "clear-scope",
            "clear_active_scope",
            main_file,
            215,
            "fn clear_active_scope(state: &AppState) { state.query_cache.clear(); }",
        ));
        graph.add_node(test_node(
            "app-state",
            "AppState",
            state_file,
            25,
            "pub struct AppState { pub query_cache: Arc<QueryCache>, pub config: Arc<Config> }",
        ));
        graph.add_node(test_node(
            "query-cache",
            "QueryCache",
            cache_file,
            14,
            "pub struct QueryCache { entries: DashMap<String, CacheEntry> }",
        ));
        graph.add_node(test_node(
            "cache-entry",
            "CacheEntry",
            cache_file,
            32,
            "pub struct CacheEntry { value: Value, hit_rate: f64 }",
        ));
        graph.add_node(test_node(
            "hit-rate",
            "hit_rate",
            cache_file,
            60,
            "fn hit_rate(entry: &CacheEntry) -> f64 { entry.hit_rate }",
        ));

        add_call(&graph, "server", "dispatch");
        add_call(&graph, "dispatch", "call-tool");
        add_call(&graph, "dispatch", "app-state");
        add_call(&graph, "app-state", "query-cache");
        add_call(&graph, "query-cache", "cache-entry");
        add_call(&graph, "handle-status", "app-state");
        add_call(&graph, "handle-status", "query-cache");
        add_call(&graph, "clear-scope", "app-state");
        add_call(&graph, "clear-scope", "query-cache");
        add_call(&graph, "hit-rate", "cache-entry");

        let result = edit_plan(
            &json!({"goal":"add per-tool result caching to the dispatch function using QueryCache"}),
            &graph,
            Some("/tmp/contextro"),
        );

        let affected = result["affected_symbols"].as_array().unwrap();
        let names: Vec<&str> = affected
            .iter()
            .filter_map(|symbol| symbol["name"].as_str())
            .collect();
        let target_files: Vec<&str> = result["target_files"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str())
            .collect();
        let app_state_index = names.iter().position(|name| *name == "AppState");
        let handle_status_index = names.iter().position(|name| *name == "handle_status");
        let clear_scope_index = names.iter().position(|name| *name == "clear_active_scope");

        assert!(names.contains(&"dispatch"), "unexpected names: {:?}", names);
        assert!(names.contains(&"QueryCache"), "unexpected names: {:?}", names);
        assert!(names.contains(&"AppState"), "unexpected names: {:?}", names);
        assert!(names.contains(&"call_tool") || names.contains(&"ContextroServer"), "unexpected names: {:?}", names);
        assert!(handle_status_index.is_none(), "unexpected names: {:?}", names);
        assert!(clear_scope_index.is_none(), "unexpected names: {:?}", names);
        assert!(target_files.contains(&"crates/contextro-server/src/main.rs"), "unexpected target files: {:?}", target_files);
        assert!(target_files.contains(&"crates/contextro-server/src/state.rs"), "unexpected target files: {:?}", target_files);
        assert!(target_files.contains(&"crates/contextro-engines/src/cache.rs"), "unexpected target files: {:?}", target_files);
        if let (Some(app_state_index), Some(call_tool_index)) = (
            app_state_index,
            names.iter().position(|name| *name == "call_tool"),
        ) {
            assert!(app_state_index < call_tool_index, "unexpected ordering: {:?}", names);
        }
        if let (Some(app_state_index), Some(server_index)) = (
            app_state_index,
            names.iter().position(|name| *name == "ContextroServer"),
        ) {
            assert!(app_state_index < server_index, "unexpected ordering: {:?}", names);
        }
    }

    #[test]
    fn test_search_codebase_map_surfaces_conceptual_cluster() {
        let graph = CodeGraph::new();
        let file = "/tmp/contextro/crates/contextro-tools/src/search.rs";
        graph.add_node(test_node(
            "handle-search",
            "handle_search",
            file,
            17,
            "pub fn handle_search() { rerank_natural_language_results(); }",
        ));
        graph.add_node(test_node(
            "rerank",
            "rerank_natural_language_results",
            file,
            21,
            "fn rerank_natural_language_results() { score search ranking candidates }",
        ));
        graph.add_node(test_node(
            "drop-noise",
            "drop_low_confidence_noise",
            file,
            130,
            "fn drop_low_confidence_noise() { filter weak ranking results }",
        ));
        graph.add_node(test_node(
            "lookup-query",
            "is_symbol_lookup_query",
            file,
            386,
            "fn is_symbol_lookup_query() { detect symbol search queries }",
        ));
        graph.add_node(test_node(
            "vector-limit",
            "vector_candidate_limit",
            file,
            552,
            "fn vector_candidate_limit() -> usize { ranking search candidates.len() }",
        ));
        graph.add_node(test_node(
            "symbol-match",
            "result_matches_symbol_query",
            file,
            398,
            "fn result_matches_symbol_query() { score whether search results match the symbol query }",
        ));
        graph.add_node(test_node(
            "resolve-targets",
            "resolve_refactor_targets",
            "/tmp/contextro/crates/contextro-tools/src/refactor.rs",
            80,
            "fn resolve_refactor_targets() { resolve targets for edit plans }",
        ));
        graph.add_node(test_node(
            "rank-degree",
            "rank_nodes_by_degree",
            "/tmp/contextro/crates/contextro-engines/src/graph.rs",
            44,
            "fn rank_nodes_by_degree() { rank graph nodes by degree }",
        ));
        add_call(&graph, "handle-search", "rerank");
        add_call(&graph, "handle-search", "drop-noise");
        add_call(&graph, "handle-search", "lookup-query");
        add_call(&graph, "drop-noise", "lookup-query");
        add_call(&graph, "drop-noise", "symbol-match");
        add_call(&graph, "rerank", "vector-limit");
        add_call(&graph, "rank-degree", "rerank");

        let result = search_codebase_map(
            &json!({"query":"how does search ranking work"}),
            &graph,
            Some("/tmp/contextro"),
        );

        assert_eq!(result["total_files"], 1);
        assert!(result["total_symbols"].as_u64().unwrap_or(0) >= 3);
        let names: Vec<&str> = result["files"][0]["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|symbol| symbol["name"].as_str())
            .collect();
        assert_eq!(
            result["files"][0]["file"],
            "crates/contextro-tools/src/search.rs"
        );
        assert!(names.contains(&"handle_search"));
        assert!(names.contains(&"rerank_natural_language_results"));
        assert!(names.contains(&"drop_low_confidence_noise"));
        assert!(names.contains(&"is_symbol_lookup_query"));
        assert!(names.contains(&"result_matches_symbol_query"));
        assert!(names.contains(&"vector_candidate_limit"));
        assert!(
            !names.contains(&"resolve_refactor_targets"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            !names.contains(&"rank_nodes_by_degree"),
            "unexpected names: {:?}",
            names
        );
    }

    #[test]
    fn test_search_codebase_map_prefers_product_surface_over_engine_internals() {
        let graph = CodeGraph::new();
        let tool_file = "/tmp/contextro/crates/contextro-tools/src/search.rs";
        let engine_file = "/tmp/contextro/crates/contextro-engines/src/search.rs";

        graph.add_node(test_node(
            "tool-handle-search",
            "handle_search",
            tool_file,
            17,
            "pub fn handle_search() { rerank_natural_language_results(); drop_low_confidence_noise(); }",
        ));
        graph.add_node(test_node(
            "tool-rerank",
            "rerank_natural_language_results",
            tool_file,
            216,
            "fn rerank_natural_language_results() { improve search ranking for product responses }",
        ));
        graph.add_node(test_node(
            "tool-noise",
            "drop_low_confidence_noise",
            tool_file,
            130,
            "fn drop_low_confidence_noise() { prune weak ranking results from tool output }",
        ));
        graph.add_node(test_node(
            "engine-execute",
            "execute_search",
            engine_file,
            61,
            "pub fn execute_search() { fuse(); }",
        ));
        graph.add_node(test_node(
            "engine-fuse",
            "fuse",
            "/tmp/contextro/crates/contextro-engines/src/fusion.rs",
            29,
            "fn fuse() { adaptive_weights(); }",
        ));
        graph.add_node(test_node(
            "engine-adaptive",
            "adaptive_weights",
            "/tmp/contextro/crates/contextro-engines/src/fusion.rs",
            85,
            "fn adaptive_weights() { rank vector and bm25 search inputs }",
        ));
        add_call(&graph, "tool-handle-search", "tool-rerank");
        add_call(&graph, "tool-handle-search", "tool-noise");
        add_call(&graph, "engine-execute", "engine-fuse");
        add_call(&graph, "engine-fuse", "engine-adaptive");

        let result = search_codebase_map(
            &json!({"query":"how does search ranking work"}),
            &graph,
            Some("/tmp/contextro"),
        );

        assert_eq!(
            result["files"][0]["file"],
            "crates/contextro-tools/src/search.rs"
        );
        let names: Vec<&str> = result["files"][0]["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|symbol| symbol["name"].as_str())
            .collect();
        assert!(
            names.contains(&"handle_search"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            names.contains(&"rerank_natural_language_results"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            names.contains(&"drop_low_confidence_noise"),
            "unexpected names: {:?}",
            names
        );
    }

    #[test]
    fn test_search_codebase_map_prefers_dominant_same_file_subsystem_closure() {
        let graph = CodeGraph::new();
        let search_file = "/tmp/contextro/crates/contextro-tools/src/search.rs";

        graph.add_node(test_node(
            "handle-search",
            "handle_search",
            search_file,
            17,
            "pub fn handle_search() { rerank_natural_language_results(); drop_low_confidence_noise(); }",
        ));
        graph.add_node(test_node(
            "rerank",
            "rerank_natural_language_results",
            search_file,
            99,
            "fn rerank_natural_language_results() { search ranking response output rerank results }",
        ));
        graph.add_node(test_node(
            "drop-noise",
            "drop_low_confidence_noise",
            search_file,
            130,
            "fn drop_low_confidence_noise() { search ranking noise output response filtering }",
        ));
        graph.add_node(test_node(
            "lookup-query",
            "is_symbol_lookup_query",
            search_file,
            386,
            "fn is_symbol_lookup_query() -> bool { detect search symbol query before ranking }",
        ));
        graph.add_node(test_node(
            "symbol-match",
            "result_matches_symbol_query",
            search_file,
            398,
            "fn result_matches_symbol_query() { keep search ranking results that match the symbol query }",
        ));
        graph.add_node(test_node(
            "vector-limit",
            "vector_candidate_limit",
            search_file,
            552,
            "fn vector_candidate_limit() -> usize { choose search ranking candidate limits for result reranking }",
        ));
        graph.add_node(test_node(
            "query-targets",
            "query_targets_product_surface",
            search_file,
            304,
            "fn query_targets_product_surface(query: &str) -> bool { detect whether search asks about product surface output }",
        ));
        graph.add_node(test_node(
            "fuse-results",
            "fuse_results",
            search_file,
            479,
            "fn fuse_results(query: &str) { accumulate_result(query); }",
        ));
        graph.add_node(test_node(
            "accumulate-result",
            "accumulate_result",
            search_file,
            565,
            "fn accumulate_result() { update fused score maps }",
        ));

        graph.add_node(test_node(
            "resolve-targets",
            "resolve_refactor_targets",
            "/tmp/contextro/crates/contextro-tools/src/refactor.rs",
            80,
            "fn resolve_refactor_targets() { resolve targets for edit plan changes }",
        ));
        graph.add_node(test_node(
            "rank-degree",
            "rank_nodes_by_degree",
            "/tmp/contextro/crates/contextro-engines/src/graph.rs",
            44,
            "fn rank_nodes_by_degree() { graph ranking nodes by degree for connectivity }",
        ));
        graph.add_node(test_node(
            "query-targets",
            "query_targets_product_surface",
            "/tmp/contextro/crates/contextro-tools/src/code.rs",
            80,
            "fn query_targets_product_surface() -> bool { detect whether the query asks about tool output }",
        ));

        add_call(&graph, "handle-search", "rerank");
        add_call(&graph, "handle-search", "drop-noise");
        add_call(&graph, "handle-search", "lookup-query");
        add_call(&graph, "drop-noise", "lookup-query");
        add_call(&graph, "drop-noise", "symbol-match");
        add_call(&graph, "rerank", "vector-limit");
        add_call(&graph, "rerank", "fuse-results");
        add_call(&graph, "fuse-results", "accumulate-result");
        add_call(&graph, "query-targets", "handle-search");
        add_call(&graph, "rank-degree", "rerank");
        add_call(&graph, "query-targets", "handle-search");

        let result = search_codebase_map(
            &json!({"query":"how does search ranking work"}),
            &graph,
            Some("/tmp/contextro"),
        );

        assert_eq!(result["total_files"], 1, "unexpected result: {result}");
        assert_eq!(
            result["files"][0]["file"],
            "crates/contextro-tools/src/search.rs"
        );

        let names: Vec<&str> = result["files"][0]["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|symbol| symbol["name"].as_str())
            .collect();

        assert!(
            names.contains(&"handle_search"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            names.contains(&"rerank_natural_language_results"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            names.contains(&"drop_low_confidence_noise"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            names.contains(&"is_symbol_lookup_query"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            names.contains(&"result_matches_symbol_query"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            names.contains(&"vector_candidate_limit"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            !names.contains(&"resolve_refactor_targets"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            !names.contains(&"rank_nodes_by_degree"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            !names.contains(&"query_targets_product_surface"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            !names.contains(&"fuse_results"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            !names.contains(&"accumulate_result"),
            "unexpected names: {:?}",
            names
        );
    }

    #[test]
    fn test_search_codebase_map_prefers_cache_module_for_explanatory_cache_queries() {
        let graph = CodeGraph::new();
        let cache_file = "/tmp/contextro/crates/contextro-engines/src/cache.rs";
        let tool_file = "/tmp/contextro/crates/contextro-tools/src/search.rs";

        graph.add_node(test_node(
            "query-cache",
            "QueryCache",
            cache_file,
            14,
            "pub struct QueryCache { ttl eviction invalidation cached query search responses }",
        ));
        graph.add_node(test_node(
            "cache-get",
            "QueryCache.get",
            cache_file,
            34,
            "fn get(&self, query: &str) -> Option<Value> { ttl expiry returns cached response }",
        ));
        graph.add_node(test_node(
            "cache-put",
            "QueryCache.put",
            cache_file,
            49,
            "fn put(&self, query: &str, result: Value) { cache eviction removes oldest entry at capacity }",
        ));
        graph.add_node(test_node(
            "handle-search",
            "handle_search",
            tool_file,
            17,
            "pub fn handle_search() { execute_search(); rerank_natural_language_results(); }",
        ));
        graph.add_node(test_node(
            "rerank",
            "rerank_natural_language_results",
            tool_file,
            216,
            "fn rerank_natural_language_results() { improve search ranking for product responses }",
        ));

        add_call(&graph, "query-cache", "cache-get");
        add_call(&graph, "query-cache", "cache-put");
        add_call(&graph, "handle-search", "rerank");

        let result = search_codebase_map(
            &json!({"query":"how does the query cache work, TTL eviction"}),
            &graph,
            Some("/tmp/contextro"),
        );

        assert_eq!(result["total_files"], 1, "unexpected result: {result}");
        assert_eq!(
            result["files"][0]["file"],
            "crates/contextro-engines/src/cache.rs"
        );
        let names: Vec<&str> = result["files"][0]["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|symbol| symbol["name"].as_str())
            .collect();
        assert!(
            names.contains(&"QueryCache"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            names.contains(&"QueryCache.get"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            names.contains(&"QueryCache.put"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            !names.contains(&"handle_search"),
            "unexpected names: {:?}",
            names
        );
    }

    #[test]
    fn test_codebase_map_stemming_restores_cache_from_caching_queries() {
        let tokens = tokenize_codebase_map_text("how does caching work");

        assert!(tokens.contains(&"caching".to_string()));
        assert!(tokens.contains(&"cache".to_string()));
    }

    #[test]
    fn test_search_codebase_map_avoids_padding_narrow_commit_search_queries() {
        let graph = CodeGraph::new();
        let commit_file = "/tmp/contextro/crates/contextro-tools/src/git.rs";
        let search_file = "/tmp/contextro/crates/contextro-tools/src/search.rs";
        let code_file = "/tmp/contextro/crates/contextro-tools/src/code.rs";

        graph.add_node(test_node(
            "handle-commit-search",
            "handle_commit_search",
            commit_file,
            12,
            "pub fn handle_commit_search() { search_commit_history(); index_commit_messages(); }",
        ));
        graph.add_node(test_node(
            "search-commit-history",
            "search_commit_history",
            commit_file,
            48,
            "fn search_commit_history() { commit search history ranking query }",
        ));
        graph.add_node(test_node(
            "index-commit-messages",
            "index_commit_messages",
            commit_file,
            88,
            "fn index_commit_messages() { index commit messages for search history }",
        ));

        graph.add_node(test_node(
            "handle-search",
            "handle_search",
            search_file,
            17,
            "pub fn handle_search() { rerank_results(); search query results }",
        ));
        graph.add_node(test_node(
            "rerank-results",
            "rerank_results",
            search_file,
            120,
            "fn rerank_results() { search ranking output results }",
        ));
        graph.add_node(test_node(
            "search-codebase-map",
            "search_codebase_map",
            code_file,
            220,
            "fn search_codebase_map() { search query symbols files }",
        ));

        add_call(&graph, "handle-commit-search", "search-commit-history");
        add_call(&graph, "search-commit-history", "index-commit-messages");
        add_call(&graph, "handle-search", "rerank-results");

        let result = search_codebase_map(
            &json!({"query":"how does commit search work"}),
            &graph,
            Some("/tmp/contextro"),
        );

        assert_eq!(result["total_files"], 1, "unexpected result: {result}");
        assert_eq!(
            result["files"][0]["file"],
            "crates/contextro-tools/src/git.rs"
        );

        let names: Vec<&str> = result["files"][0]["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|symbol| symbol["name"].as_str())
            .collect();
        assert!(
            names.contains(&"handle_commit_search"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            names.contains(&"search_commit_history"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            !names.contains(&"handle_search"),
            "unexpected names: {:?}",
            names
        );
        assert!(
            !names.contains(&"search_codebase_map"),
            "unexpected names: {:?}",
            names
        );
    }

    #[test]
    fn test_search_codebase_map_keeps_broad_architectural_queries_multi_file() {
        let graph = CodeGraph::new();
        let tool_file = "/tmp/contextro/crates/contextro-tools/src/git.rs";
        let git_file = "/tmp/contextro/crates/contextro-git/src/commit_index.rs";

        graph.add_node(test_node(
            "handle-commit-search",
            "handle_commit_search",
            tool_file,
            12,
            "pub fn handle_commit_search() { search_commit_history(); }",
        ));
        graph.add_node(test_node(
            "search-commit-history",
            "search_commit_history",
            tool_file,
            48,
            "fn search_commit_history() { commit search architecture pipeline routes queries to git history }",
        ));
        graph.add_node(test_node(
            "commit-index",
            "CommitIndex",
            git_file,
            20,
            "pub struct CommitIndex { commit search architecture pipeline storage }",
        ));
        graph.add_node(test_node(
            "search-commit-messages",
            "search_commit_messages",
            git_file,
            75,
            "fn search_commit_messages() { commit search architecture pipeline ranking }",
        ));

        add_call(&graph, "handle-commit-search", "search-commit-history");
        add_call(&graph, "search-commit-history", "search-commit-messages");

        let result = search_codebase_map(
            &json!({"query":"commit search architecture pipeline"}),
            &graph,
            Some("/tmp/contextro"),
        );

        assert!(
            result["total_files"].as_u64().unwrap_or(0) >= 2,
            "unexpected result: {result}"
        );
        let files: Vec<&str> = result["files"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|file| file["file"].as_str())
            .collect();
        assert!(
            files.contains(&"crates/contextro-tools/src/git.rs"),
            "unexpected files: {:?}",
            files
        );
        assert!(
            files.contains(&"crates/contextro-git/src/commit_index.rs"),
            "unexpected files: {:?}",
            files
        );
    }

    #[test]
    fn test_get_document_symbols_omits_signatures_by_default() {
        let dir = temp_dir("default-signature");
        let file = dir.join("main.py");
        std::fs::write(&file, "def hello(name):\n    return name\n").unwrap();

        let result = get_document_symbols(
            &json!({"path": file.to_string_lossy().to_string()}),
            Some(dir.to_string_lossy().as_ref()),
        );

        assert_eq!(result["total"], 1);
        assert_eq!(result["columns"], json!(["name", "type", "line"]));
        assert_eq!(result["symbols"][0], json!(["hello", "function", 1]));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_get_document_symbols_truncates_unicode_signatures_when_requested() {
        let dir = temp_dir("unicode-signature");
        let file = dir.join("main.py");
        let signature = format!("def hello({}) -> str:", "─".repeat(80));
        std::fs::write(&file, format!("{signature}\n    return 'ok'\n")).unwrap();

        let result = get_document_symbols(
            &json!({
                "path": file.to_string_lossy().to_string(),
                "include_signature": true
            }),
            Some(dir.to_string_lossy().as_ref()),
        );

        assert_eq!(result["total"], 1);
        assert_eq!(result["columns"], json!(["name", "type", "line", "signature"]));
        let rendered = result["symbols"][0][3].as_str().unwrap();
        assert!(rendered.ends_with('…'));
        assert!(rendered.chars().count() <= 58);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_get_document_symbols_uses_shared_columns_for_multiline_symbols() {
        let dir = temp_dir("shared-columns");
        let file = dir.join("main.py");
        std::fs::write(
            &file,
            "class Hello:\n    def first(self):\n        return 1\n\ndef second():\n    return 2\n",
        )
        .unwrap();

        let result = get_document_symbols(
            &json!({"path": file.to_string_lossy().to_string()}),
            Some(dir.to_string_lossy().as_ref()),
        );

        assert_eq!(result["columns"], json!(["name", "type", "line", "end_line"]));
        assert_eq!(result["symbols"][0], json!(["Hello", "class", 1, 3]));
        assert_eq!(result["symbols"][1], json!(["first", "method", 2, null]));
        assert_eq!(result["symbols"][2], json!(["second", "function", 5, null]));

        let _ = std::fs::remove_dir_all(dir);
    }

}
