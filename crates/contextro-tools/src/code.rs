//! Code tool: AST operations dispatch.

use std::cmp::Ordering;
use std::collections::HashSet;
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
            let symbols: Vec<Value> = parsed
                .symbols
                .iter()
                .map(|s| {
                    let mut sym = json!({
                        "name": s.name,
                        "type": s.symbol_type.to_string(),
                        "line": s.line_start,
                    });
                    // Only include end_line if multi-line
                    if s.line_end > s.line_start + 1 {
                        sym["end_line"] = json!(s.line_end);
                    }
                    // Truncate long signatures to save tokens
                    let sig = if s.signature.chars().count() > 60 {
                        truncate_chars(&s.signature, 57)
                    } else {
                        s.signature.clone()
                    };
                    if !sig.is_empty() {
                        sym["signature"] = json!(sig);
                    }
                    sym
                })
                .collect();
            json!({"file": strip_base(&abs_path.to_string_lossy(), codebase), "symbols": symbols, "total": symbols.len()})
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
    if !Path::new(&target).exists() {
        let missing = file_path.or(search_path).unwrap_or(&target);
        return json!({"error": format!("Path not found: {}", missing)});
    }

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
    if !Path::new(&target).exists() {
        let missing = file_path.or(search_path).unwrap_or(&target);
        return json!({"error": format!("Path not found: {}", missing)});
    }

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
    let symbol_name = args
        .get("symbol_name")
        .or_else(|| args.get("name"))
        .and_then(|v| v.as_str());

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
            let matches = graph.find_nodes_by_name(&test_name, false);
            if matches.is_empty() || matches.iter().all(|node| node.name != test_name) {
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
    struct CodebaseMapHit {
        file: String,
        score: f64,
        is_test_like: bool,
        symbol: Value,
    }

    let raw_query = args
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let normalized_query = raw_query.to_ascii_lowercase();
    let query_tokens = tokenize_codebase_map_text(raw_query);
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
        let score = codebase_map_match_score(node, &normalized_query, &query_tokens);
        if !normalized_query.is_empty() && score <= 0.0 {
            continue;
        }
        let rel = strip_base(&node.location.file_path, codebase);
        let (callers, callees) = graph.get_node_degree(&node.id);
        hits.push(CodebaseMapHit {
            is_test_like: is_test_file(&node.location.file_path)
                || is_probable_codebase_map_test_symbol(&node.name),
            file: rel,
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

    if !codebase_map_query_targets_tests(raw_query) && hits.iter().any(|hit| !hit.is_test_like) {
        hits.retain(|hit| !hit.is_test_like);
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
    }

    let files: Vec<Value> = grouped
        .into_iter()
        .map(|(file, mut symbols, _)| {
            if normalized_query.is_empty() {
                symbols.sort_by(|a, b| {
                    a.symbol["line"]
                        .as_u64()
                        .unwrap_or(0)
                        .cmp(&b.symbol["line"].as_u64().unwrap_or(0))
                });
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
    codebase
        .and_then(|b| Path::new(file).strip_prefix(b).ok())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| file.to_string())
}

fn codebase_map_match_score(
    node: &UniversalNode,
    normalized_query: &str,
    query_tokens: &[String],
) -> f64 {
    if normalized_query.is_empty() {
        return 1.0;
    }

    let qualified_name = node
        .parent
        .as_ref()
        .map(|parent| format!("{parent}.{}", node.name))
        .unwrap_or_else(|| node.name.clone());
    let fields = [
        node.name.as_str(),
        qualified_name.as_str(),
        node.location.file_path.as_str(),
        node.content.as_str(),
        node.docstring.as_deref().unwrap_or(""),
    ];

    let exact_match = fields
        .iter()
        .any(|field| field.to_ascii_lowercase().contains(normalized_query));
    let candidate_tokens: HashSet<String> = fields
        .iter()
        .flat_map(|field| tokenize_codebase_map_text(field))
        .collect();
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

fn required_codebase_map_matches(token_count: usize) -> usize {
    match token_count {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => token_count.div_ceil(2),
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

    normalized
        .split_whitespace()
        .filter(|token| token.len() >= 3)
        .map(String::from)
        .collect()
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
    use contextro_core::graph::{UniversalLocation, UniversalNode};
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
    fn test_list_symbols_truncates_unicode_signatures_safely() {
        let dir = temp_dir("unicode-signature");
        let file = dir.join("main.py");
        let signature = format!("def hello({}) -> str:", "─".repeat(80));
        std::fs::write(&file, format!("{signature}\n    return 'ok'\n")).unwrap();

        let result = get_document_symbols(
            &json!({"path": file.to_string_lossy().to_string()}),
            Some(dir.to_string_lossy().as_ref()),
        );

        assert_eq!(result["total"], 1);
        let rendered = result["symbols"][0]["signature"].as_str().unwrap();
        assert!(rendered.ends_with('…'));
        assert!(rendered.chars().count() <= 58);

        let _ = std::fs::remove_dir_all(dir);
    }
}
