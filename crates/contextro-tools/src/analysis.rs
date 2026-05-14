//! Analysis tools: overview, architecture, analyze, focus, dead_code, circular_dependencies, test_coverage_map.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use contextro_core::NodeType;
use contextro_engines::graph::CodeGraph;
use serde_json::{json, Value};

/// Generic/noise names filtered from architecture hub rankings, dead_code, and high-connectivity reports.
/// Includes Rust stdlib methods, JS/TS test framework globals, and common single-word identifiers
/// that appear everywhere and inflate graph metrics without conveying architectural meaning.
const GENERIC_NAMES: &[&str] = &[
    // Rust stdlib
    "new",
    "default",
    "clone",
    "drop",
    "fmt",
    "from",
    "into",
    "as_ref",
    "as_mut",
    "len",
    "is_empty",
    "iter",
    "iter_mut",
    "get",
    "get_mut",
    "set",
    "push",
    "append",
    "pop",
    "insert",
    "remove",
    "contains",
    "clear",
    "extend",
    "collect",
    "map",
    "filter",
    "unwrap",
    "unwrap_or",
    "expect",
    "ok",
    "err",
    "to_string",
    "to_owned",
    "parse",
    "deref",
    "deref_mut",
    "send",
    "recv",
    "read",
    "write",
    "flush",
    "close",
    // JS/TS test framework globals (Jest, Vitest, Mocha, Playwright)
    "describe",
    "it",
    "test",
    "expect",
    "beforeEach",
    "afterEach",
    "beforeAll",
    "afterAll",
    "vi",
    "jest",
    "assert",
    "suite",
    "bench",
    // JS/TS language keywords misidentified as symbols
    "export",
    "await",
    "async",
    "return",
    "import",
    "require",
    // Common single-word JS identifiers that appear in every file
    "id",
    "name",
    "type",
    "value",
    "data",
    "result",
    "error",
    "now",
    "next",
    "key",
    "index",
    "item",
    "node",
    "ref",
    "props",
    "state",
    "ctx",
    "res",
    "req",
    "number",
    "string",
    "boolean",
    "object",
    "array",
];

pub(crate) fn is_generic_symbol_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    GENERIC_NAMES.contains(&name)
        || GENERIC_NAMES.contains(&lower.as_str())
        || (name.len() > 4 && name.starts_with("__") && name.ends_with("__"))
}

pub fn handle_overview(
    graph: &CodeGraph,
    codebase: Option<&str>,
    total_chunks: usize,
    vector_chunks: usize,
) -> Value {
    let nodes = graph.find_nodes_by_name("", false);
    let node_count = nodes.len();
    let rel_count = graph.relationship_count();
    let mut file_counts: HashMap<String, usize> = HashMap::new();
    let mut language_counts: HashMap<String, usize> = HashMap::new();
    let mut type_counts: HashMap<String, usize> = HashMap::new();
    let mut directory_counts: HashMap<String, usize> = HashMap::new();

    for node in &nodes {
        *file_counts
            .entry(node.location.file_path.clone())
            .or_default() += 1;
        *language_counts.entry(node.language.clone()).or_default() += 1;
        *type_counts.entry(node.node_type.to_string()).or_default() += 1;

        let directory = Path::new(&node.location.file_path)
            .parent()
            .map(|parent| parent.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".into());
        *directory_counts.entry(directory).or_default() += 1;
    }

    let languages = sort_counts(language_counts)
        .into_iter()
        .map(|(language, count)| json!({"language": language, "symbols": count}))
        .collect::<Vec<_>>();
    let symbol_types = sort_counts(type_counts)
        .into_iter()
        .map(|(symbol_type, count)| json!({"type": symbol_type, "count": count}))
        .collect::<Vec<_>>();
    let total_files = file_counts.len();
    let top_files = sort_counts(file_counts)
        .into_iter()
        .take(10)
        .map(|(file, count)| json!({"file": strip_base(&file, codebase), "symbols": count}))
        .collect::<Vec<_>>();
    let top_directories = sort_counts(directory_counts)
        .into_iter()
        .take(10)
        .map(|(directory, count)| json!({"path": strip_base(&directory, codebase), "symbols": count}))
        .collect::<Vec<_>>();

    json!({
        "codebase_path": codebase,
        "total_symbols": node_count,
        "total_relationships": rel_count,
        "total_chunks": total_chunks,
        "vector_chunks": vector_chunks,
        "total_files": total_files,
        "languages": languages,
        "symbol_types": symbol_types,
        "top_files_by_symbols": top_files,
        "top_directories": top_directories,
    })
}

pub fn handle_architecture(graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let nodes = graph.find_nodes_by_name("", false);
    let mut scored: Vec<(String, String, usize)> = nodes
        .iter()
        .filter(|n| !is_generic_symbol_name(&n.name))
        .filter(|n| !is_test_file(&n.location.file_path))
        .map(|n| {
            let (in_d, out_d) = graph.get_node_degree(&n.id);
            (n.name.clone(), n.location.file_path.clone(), in_d + out_d)
        })
        .collect();
    scored.sort_by_key(|b| std::cmp::Reverse(b.2));

    let hubs: Vec<Value> = scored
        .iter()
        .take(10)
        .map(|(name, file, degree)| {
            let fp = strip_base(file, codebase);
            json!({"name": name, "file": fp, "degree": degree})
        })
        .collect();

    json!({"hub_symbols": hubs, "total_nodes": graph.node_count(), "total_edges": graph.relationship_count()})
}

pub fn handle_analyze(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let path_filter = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let all_nodes = graph.find_nodes_by_name("", false);

    // Filter nodes to the requested path prefix if specified
    let nodes: Vec<_> = if path_filter.is_empty() {
        all_nodes
    } else {
        let abs_filter = match resolve_existing_path(path_filter, codebase) {
            Ok(path) => path,
            Err(error) => return error,
        };
        let is_dir = abs_filter.is_dir();
        all_nodes
            .into_iter()
            .filter(|n| path_matches(&n.location.file_path, &abs_filter, is_dir))
            .collect()
    };

    let mut complex_fns: Vec<Value> = Vec::new();
    let mut file_sizes: HashMap<String, usize> = HashMap::new();

    for node in &nodes {
        *file_sizes
            .entry(node.location.file_path.clone())
            .or_default() += 1;
        if is_generic_symbol_name(&node.name) || is_test_file(&node.location.file_path) {
            continue;
        }
        let (in_d, out_d) = graph.get_node_degree(&node.id);
        if in_d + out_d > 5 {
            complex_fns.push(json!({"name": node.name, "file": strip_base(&node.location.file_path, codebase), "connections": in_d + out_d}));
        }
    }
    complex_fns.sort_by_key(|v| std::cmp::Reverse(v["connections"].as_u64().unwrap_or(0)));
    complex_fns.truncate(10);

    let mut large_files: Vec<Value> = file_sizes
        .iter()
        .filter(|(_, count)| **count > 10)
        .map(|(file, count)| json!({"file": strip_base(file, codebase), "symbols": count}))
        .collect();
    large_files.sort_by_key(|v| std::cmp::Reverse(v["symbols"].as_u64().unwrap_or(0)));

    json!({"path": if path_filter.is_empty() { Value::Null } else { json!(path_filter) }, "high_connectivity_symbols": complex_fns, "large_files": large_files, "total_symbols": nodes.len()})
}

/// Low-token context slice for a single file.
pub fn handle_focus(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return json!({"error": "Missing required parameter: path"});
    }

    let abs_path = match resolve_existing_path(path, codebase) {
        Ok(path) => path,
        Err(error) => return error,
    };

    let nodes = graph.find_nodes_by_name("", false);

    // Directory: list top symbols grouped by file
    if abs_path.is_dir() {
        let mut by_file: std::collections::BTreeMap<String, Vec<Value>> =
            std::collections::BTreeMap::new();
        for n in nodes
            .iter()
            .filter(|n| path_matches(&n.location.file_path, &abs_path, true))
        {
            let (in_d, out_d) = graph.get_node_degree(&n.id);
            by_file.entry(strip_base(&n.location.file_path, codebase)).or_default().push(
                json!({"name": n.name, "type": n.node_type.to_string(), "line": n.location.start_line, "callers": in_d, "callees": out_d})
            );
        }
        let total_symbols: usize = by_file.values().map(|v| v.len()).sum();
        let files: Vec<Value> = by_file
            .into_iter()
            .map(|(file, syms)| json!({"file": file, "symbols": syms}))
            .collect();
        return json!({
            "path": strip_base(&abs_path.to_string_lossy(), codebase),
            "is_directory": true,
            "files": files,
            "total_symbols": total_symbols,
        });
    }

    // Single file
    let file_symbols: Vec<Value> = nodes.iter()
        .filter(|n| n.location.file_path == abs_path.to_string_lossy())
        .map(|n| {
            let (in_d, out_d) = graph.get_node_degree(&n.id);
            json!({"name": n.name, "type": n.node_type.to_string(), "line": n.location.start_line, "callers": in_d, "callees": out_d})
        })
        .collect();

    let preview = std::fs::read_to_string(&abs_path)
        .map(|s| s.lines().take(5).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default();

    json!({
        "file": strip_base(&abs_path.to_string_lossy(), codebase),
        "symbols": file_symbols,
        "total_symbols": file_symbols.len(),
        "preview": preview,
    })
}

/// Dead code analysis: find symbols with zero callers that aren't entry points.
pub fn handle_dead_code(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let nodes = graph.find_nodes_by_name("", false);
    let mut dead: Vec<Value> = Vec::new();
    let mut file_cache: HashMap<String, Option<String>> = HashMap::new();
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

    for node in &nodes {
        // Skip classes and variables — focus on functions/methods
        if node.node_type != NodeType::Function {
            continue;
        }
        if let Some((target_path, is_dir)) = &path_filter {
            if !path_matches(&node.location.file_path, target_path, *is_dir) {
                continue;
            }
        }
        if excluded_paths.iter().any(|(target_path, is_dir)| {
            path_matches(&node.location.file_path, target_path, *is_dir)
        }) {
            skipped_excluded += 1;
            continue;
        }
        if !include_tests && is_test_file(&node.location.file_path) {
            skipped_tests += 1;
            continue;
        }
        let (in_degree, _) = graph.get_node_degree(&node.id);
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

/// Circular dependency detection at the file/module import level.
/// Scans `use crate::` and `use super::` statements — not call edges — to avoid
/// false positives from normal cross-module function calls.
pub fn handle_circular_dependencies(graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let nodes = graph.find_nodes_by_name("", false);
    let mut all_files: HashSet<String> = HashSet::new();
    for node in &nodes {
        all_files.insert(node.location.file_path.clone());
    }

    let mut file_deps: HashMap<String, HashSet<String>> = HashMap::new();

    for file_path in &all_files {
        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for line in content.lines() {
            let trimmed = line.trim();

            // Rust: use crate:: / use super::
            if file_path.ends_with(".rs") {
                let after_prefix = trimmed
                    .strip_prefix("use crate::")
                    .or_else(|| trimmed.strip_prefix("use super::"));
                if let Some(after) = after_prefix {
                    let segment = after
                        .split([':', ';', ' ', '{', ','])
                        .next()
                        .unwrap_or("")
                        .trim();
                    if !segment.is_empty() {
                        if let Some(dep) = all_files.iter().find(|f| {
                            let stem = Path::new(f)
                                .file_stem()
                                .map(|s| s.to_string_lossy().to_string());
                            stem.as_deref() == Some(segment) && f.as_str() != file_path.as_str()
                        }) {
                            file_deps
                                .entry(file_path.clone())
                                .or_default()
                                .insert(dep.clone());
                        }
                    }
                }
            }

            // TypeScript/JavaScript: import ... from './relative/path'
            if (file_path.ends_with(".ts")
                || file_path.ends_with(".tsx")
                || file_path.ends_with(".js")
                || file_path.ends_with(".jsx"))
                && trimmed.starts_with("import ")
            {
                // Extract the path from: import ... from './foo' or import ... from '../bar'
                if let Some(from_pos) = trimmed.find("from ") {
                    let after_from = &trimmed[from_pos + 5..];
                    let path_str = after_from
                        .trim_start_matches(['\'', '"'])
                        .trim_end_matches(['\'', '"', ';']);
                    // Only relative imports can form cycles
                    if path_str.starts_with('.') {
                        let dir = Path::new(file_path).parent().unwrap_or(Path::new(""));
                        let resolved = dir.join(path_str);
                        // Try common extensions
                        let candidates = [
                            resolved.with_extension("ts"),
                            resolved.with_extension("tsx"),
                            resolved.with_extension("js"),
                            resolved.with_extension("jsx"),
                            resolved.join("index.ts"),
                            resolved.join("index.tsx"),
                        ];
                        for candidate in &candidates {
                            let cand_str = candidate.to_string_lossy().to_string();
                            if all_files.contains(&cand_str) && cand_str != *file_path {
                                file_deps
                                    .entry(file_path.clone())
                                    .or_default()
                                    .insert(cand_str);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    let files: Vec<String> = file_deps.keys().cloned().collect();
    let sccs = tarjan_scc(&files, &file_deps);

    let cycles: Vec<Value> = sccs
        .iter()
        .filter(|scc| scc.len() > 1)
        .map(|scc| {
            let files: Vec<String> = scc.iter().map(|f| strip_base(f, codebase)).collect();
            json!({"files": files, "size": scc.len()})
        })
        .collect();

    json!({"circular_dependencies": cycles, "total": cycles.len()})
}

/// Static test coverage map.
/// Recognises test files by: test_*.rs, *_test.rs, *.test.ts/tsx, *.spec.ts/tsx,
/// __tests__/ directories, tests/ directories, and Rust inline #[cfg(test)] blocks.
pub fn handle_test_coverage_map(graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let nodes = graph.find_nodes_by_name("", false);
    let mut source_files: HashSet<String> = HashSet::new();
    let mut test_files: HashSet<String> = HashSet::new();
    let mut source_tokens: HashMap<String, HashSet<String>> = HashMap::new();
    let mut test_file_tokens: HashMap<String, HashSet<String>> = HashMap::new();

    for node in &nodes {
        let fp = &node.location.file_path;
        let relative_path = strip_base(fp, codebase);
        if is_test_file(fp) {
            test_files.insert(fp.clone());
            let entry = test_file_tokens.entry(fp.clone()).or_default();
            entry.extend(coverage_tokens(&relative_path));
            entry.extend(coverage_tokens(&node.name));
        } else {
            source_files.insert(fp.clone());
            source_tokens
                .entry(fp.clone())
                .or_insert_with(|| coverage_tokens(&relative_path));
        }
    }

    let mut source_token_frequency: HashMap<String, usize> = HashMap::new();
    for tokens in source_tokens.values() {
        for token in tokens {
            *source_token_frequency.entry(token.clone()).or_default() += 1;
        }
    }

    // Rust inline test modules
    let mut inline_tested: HashSet<String> = HashSet::new();
    for fp in &source_files {
        if fp.ends_with(".rs") {
            if let Ok(content) = std::fs::read_to_string(fp) {
                if content.contains("#[cfg(test)]") || content.contains("#[test]") {
                    inline_tested.insert(fp.clone());
                }
            }
        }
    }

    let mut covered_exact: Vec<String> = Vec::new();
    let mut covered: Vec<String> = Vec::new();
    let mut uncovered: Vec<String> = Vec::new();
    let mut likely_covered: Vec<String> = Vec::new();

    for src in &source_files {
        let src_stem = file_stem_stripped(src);

        let exact_match = inline_tested.contains(src)
            || test_files.iter().any(|t| {
                let t_stem = file_stem_stripped(t);
                t_stem == src_stem
                    || t_stem == format!("test_{}", src_stem)
                    || t_stem == format!("{}_test", src_stem)
            });

        let heuristic_match = !exact_match
            && source_tokens.get(src).is_some_and(|src_tokens| {
                test_file_tokens.values().any(|test_tokens| {
                    has_probable_test_signal(src_tokens, test_tokens, &source_token_frequency)
                })
            });

        if exact_match {
            covered_exact.push(strip_base(src, codebase));
            covered.push(strip_base(src, codebase));
        } else if heuristic_match {
            let path = strip_base(src, codebase);
            likely_covered.push(path.clone());
            covered.push(path);
        } else {
            uncovered.push(strip_base(src, codebase));
        }
    }

    let conservative_pct = if source_files.is_empty() {
        0.0
    } else {
        covered_exact.len() as f64 / source_files.len() as f64 * 100.0
    };
    let coverage_pct = if source_files.is_empty() {
        0.0
    } else {
        covered.len() as f64 / source_files.len() as f64 * 100.0
    };

    json!({
        "coverage_type": "static_heuristic",
        "coverage_percent": (coverage_pct * 10.0).round() / 10.0,
        "likely_coverage_percent": (coverage_pct * 10.0).round() / 10.0,
        "static_coverage_percent": (coverage_pct * 10.0).round() / 10.0,
        "conservative_coverage_percent": (conservative_pct * 10.0).round() / 10.0,
        "coverage_range_percent": {
            "lower_bound": (conservative_pct * 10.0).round() / 10.0,
            "upper_bound": (coverage_pct * 10.0).round() / 10.0,
        },
        "covered_files": covered.len(),
        "conservative_covered_files": covered_exact.len(),
        "likely_covered_files": likely_covered.len(),
        "uncovered_files": uncovered.len(),
        "test_files": test_files.len() + inline_tested.len(),
        "likely_covered": likely_covered.into_iter().take(20).collect::<Vec<_>>(),
        "uncovered": uncovered.into_iter().take(20).collect::<Vec<_>>(),
        "interpretation": "Read conservative_coverage_percent as an exact-match lower bound and coverage_percent / likely_coverage_percent as a heuristic upper bound for projects whose test files do not follow naming conventions.",
        "note": "Static heuristic based on inline tests, exact filename matches, and source/test token overlap. Treat this as directional file coverage, not runtime or line coverage.",
    })
}

fn file_stem_stripped(fp: &str) -> String {
    let stem = Path::new(fp)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    if let Some(s) = stem.strip_suffix(".test") {
        return s.to_string();
    }
    if let Some(s) = stem.strip_suffix(".spec") {
        return s.to_string();
    }
    stem
}

fn coverage_tokens(text: &str) -> HashSet<String> {
    const GENERIC_COVERAGE_TOKENS: &[&str] = &[
        "test",
        "tests",
        "spec",
        "ci",
        "e2e",
        "integration",
        "unit",
        "src",
        "lib",
        "app",
        "main",
        "index",
        "init",
        "conftest",
        "python",
        "rust",
    ];

    let mut spaced = String::with_capacity(text.len() * 2);
    let mut prev_was_lower_or_digit = false;

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && prev_was_lower_or_digit {
                spaced.push(' ');
            }
            spaced.push(ch.to_ascii_lowercase());
            prev_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else {
            spaced.push(' ');
            prev_was_lower_or_digit = false;
        }
    }

    spaced
        .split_whitespace()
        .filter(|token| token.len() >= 3)
        .filter(|token| !GENERIC_COVERAGE_TOKENS.contains(token))
        .map(String::from)
        .collect()
}

fn has_probable_test_signal(
    source_tokens: &HashSet<String>,
    test_tokens: &HashSet<String>,
    source_token_frequency: &HashMap<String, usize>,
) -> bool {
    let overlap: Vec<&String> = source_tokens.intersection(test_tokens).collect();
    if overlap.is_empty() {
        return false;
    }

    let strong_overlap = overlap
        .iter()
        .filter(|token| token.len() >= 4)
        .filter(|token| {
            source_token_frequency
                .get(token.as_str())
                .copied()
                .unwrap_or(usize::MAX)
                <= 5
        })
        .count();

    strong_overlap >= 1 || overlap.iter().filter(|token| token.len() >= 4).count() >= 2
}

fn is_probable_public_api(node: &contextro_core::UniversalNode) -> bool {
    if node.name.starts_with('_') {
        return false;
    }
    if node.parent.is_some() {
        return true;
    }
    Path::new(&node.location.file_path)
        .file_name()
        .and_then(|name| name.to_str())
        == Some("__init__.py")
}

fn parse_path_filters(
    value: Option<&Value>,
    codebase: Option<&str>,
) -> Result<Vec<(PathBuf, bool)>, Value> {
    let raw_paths: Vec<String> = match value {
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(|value| value.as_str().map(str::trim))
            .filter(|value| !value.is_empty())
            .map(String::from)
            .collect(),
        Some(Value::String(value)) => value
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(String::from)
            .collect(),
        _ => Vec::new(),
    };

    raw_paths
        .into_iter()
        .map(|path| {
            resolve_existing_path(&path, codebase).map(|resolved| {
                let is_dir = resolved.is_dir();
                (resolved, is_dir)
            })
        })
        .collect()
}

pub(crate) fn is_test_file(fp: &str) -> bool {
    let basename = Path::new(fp)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    basename.starts_with("test_")
        || basename.ends_with("_test.rs")
        || basename.contains(".test.")   // foo.test.ts, foo.test.tsx
        || basename.contains(".spec.")   // foo.spec.ts, foo.spec.tsx
        || fp.contains("/__tests__/")
        || fp.contains("/tests/")
        || fp.contains("/test/")
        || fp.contains("/e2e/")
        || fp.contains("/spec/")
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

pub(crate) fn strip_base(file: &str, codebase: Option<&str>) -> String {
    codebase
        .and_then(|b| Path::new(file).strip_prefix(b).ok())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| file.to_string())
}

fn sort_counts(counts: HashMap<String, usize>) -> Vec<(String, usize)> {
    let mut pairs: Vec<(String, usize)> = counts.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    pairs
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

fn is_pytest_fixture(
    node: &contextro_core::UniversalNode,
    file_cache: &mut HashMap<String, Option<String>>,
) -> bool {
    if node.language != "python" {
        return false;
    }

    let file_name = Path::new(&node.location.file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if file_name == "conftest.py" {
        return true;
    }

    let content = match file_cache
        .entry(node.location.file_path.clone())
        .or_insert_with(|| std::fs::read_to_string(&node.location.file_path).ok())
    {
        Some(content) => content,
        None => return false,
    };

    let lines: Vec<&str> = content.lines().collect();
    let start = node.location.start_line.saturating_sub(4) as usize;
    let end = node.location.start_line.saturating_sub(1) as usize;
    lines.get(start..end).unwrap_or(&[]).iter().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with("@pytest.fixture")
            || trimmed.starts_with("@pytest_asyncio.fixture")
            || trimmed.starts_with("@fixture")
            || trimmed.starts_with("@pytest.yield_fixture")
    })
}

/// Simple iterative Tarjan's SCC.
fn tarjan_scc(nodes: &[String], edges: &HashMap<String, HashSet<String>>) -> Vec<Vec<String>> {
    struct TarjanCtx<'a> {
        edges: &'a HashMap<String, HashSet<String>>,
        index_counter: u32,
        stack: Vec<String>,
        on_stack: HashSet<String>,
        indices: HashMap<String, u32>,
        lowlinks: HashMap<String, u32>,
        result: Vec<Vec<String>>,
    }

    impl<'a> TarjanCtx<'a> {
        fn strongconnect(&mut self, v: &str) {
            self.indices.insert(v.to_string(), self.index_counter);
            self.lowlinks.insert(v.to_string(), self.index_counter);
            self.index_counter += 1;
            self.stack.push(v.to_string());
            self.on_stack.insert(v.to_string());

            if let Some(neighbors) = self.edges.get(v) {
                for w in neighbors {
                    if !self.indices.contains_key(w.as_str()) {
                        self.strongconnect(w);
                        let wl = *self.lowlinks.get(w.as_str()).unwrap_or(&0);
                        let vl = self.lowlinks.get_mut(v).unwrap();
                        if wl < *vl {
                            *vl = wl;
                        }
                    } else if self.on_stack.contains(w.as_str()) {
                        let wi = *self.indices.get(w.as_str()).unwrap_or(&0);
                        let vl = self.lowlinks.get_mut(v).unwrap();
                        if wi < *vl {
                            *vl = wi;
                        }
                    }
                }
            }

            if self.lowlinks.get(v) == self.indices.get(v) {
                let mut scc = Vec::new();
                loop {
                    let w = self.stack.pop().unwrap();
                    self.on_stack.remove(&w);
                    scc.push(w.clone());
                    if w == v {
                        break;
                    }
                }
                self.result.push(scc);
            }
        }
    }

    let mut ctx = TarjanCtx {
        edges,
        index_counter: 0,
        stack: Vec::new(),
        on_stack: HashSet::new(),
        indices: HashMap::new(),
        lowlinks: HashMap::new(),
        result: Vec::new(),
    };

    for node in nodes {
        if !ctx.indices.contains_key(node.as_str()) {
            ctx.strongconnect(node);
        }
    }

    ctx.result
}

#[cfg(test)]
mod tests {
    use super::*;
    use contextro_core::graph::{
        RelationshipType, UniversalLocation, UniversalNode, UniversalRelationship,
    };
    use contextro_engines::graph::CodeGraph;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("contextro-analysis-{unique}-{name}"));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn test_focus_returns_error_for_missing_path() {
        let graph = CodeGraph::new();
        let result = handle_focus(&json!({"path":"missing/file.rs"}), &graph, None);
        assert!(result.get("error").is_some());
    }

    #[test]
    fn test_analyze_returns_error_for_missing_path() {
        let graph = CodeGraph::new();
        let result = handle_analyze(&json!({"path":"missing/dir"}), &graph, None);
        assert!(result.get("error").is_some());
    }

    #[test]
    fn test_dead_code_skips_pytest_fixture_functions() {
        let dir = temp_dir("fixtures");
        let file = dir.join("conftest.py");
        std::fs::write(
            &file,
            "@pytest.fixture\nasync def browser():\n    return object()\n",
        )
        .unwrap();

        let graph = CodeGraph::new();
        graph.add_node(UniversalNode {
            id: "fixture".into(),
            name: "browser".into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: file.to_string_lossy().to_string(),
                start_line: 2,
                end_line: 3,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            ..Default::default()
        });

        let result = handle_dead_code(&json!({}), &graph, Some(dir.to_string_lossy().as_ref()));
        assert_eq!(result["total"], 0);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_dead_code_skips_public_methods_unless_requested() {
        let dir = temp_dir("public-api");
        let file = dir.join("actor.py");
        std::fs::write(
            &file,
            "class BrowserSession:\n    def click(self):\n        pass\n",
        )
        .unwrap();

        let graph = CodeGraph::new();
        graph.add_node(UniversalNode {
            id: "click".into(),
            name: "click".into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: file.to_string_lossy().to_string(),
                start_line: 2,
                end_line: 3,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            parent: Some("BrowserSession".into()),
            ..Default::default()
        });

        let default_result =
            handle_dead_code(&json!({}), &graph, Some(dir.to_string_lossy().as_ref()));
        let include_public_api = handle_dead_code(
            &json!({"include_public_api": true}),
            &graph,
            Some(dir.to_string_lossy().as_ref()),
        );

        assert_eq!(default_result["total"], 0);
        assert_eq!(default_result["skipped_public_api"], 1);
        assert_eq!(include_public_api["total"], 1);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_dead_code_supports_path_and_exclude_filters() {
        let dir = temp_dir("filters");
        let src = dir.join("src");
        let vendor = dir.join("vendor");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&vendor).unwrap();
        let src_file = src.join("app.py");
        let vendor_file = vendor.join("shim.py");
        std::fs::write(&src_file, "def alpha():\n    pass\n").unwrap();
        std::fs::write(&vendor_file, "def beta():\n    pass\n").unwrap();

        let graph = CodeGraph::new();
        for (id, name, file_path) in [
            ("alpha", "alpha", src_file.to_string_lossy().to_string()),
            ("beta", "beta", vendor_file.to_string_lossy().to_string()),
        ] {
            graph.add_node(UniversalNode {
                id: id.into(),
                name: name.into(),
                node_type: NodeType::Function,
                location: UniversalLocation {
                    file_path,
                    start_line: 1,
                    end_line: 2,
                    start_column: 0,
                    end_column: 0,
                    language: "python".into(),
                },
                language: "python".into(),
                ..Default::default()
            });
        }

        let scoped = handle_dead_code(
            &json!({"path": src.to_string_lossy(), "limit": 10}),
            &graph,
            Some(dir.to_string_lossy().as_ref()),
        );
        let excluded = handle_dead_code(
            &json!({"exclude_paths": [vendor.to_string_lossy().to_string()]}),
            &graph,
            Some(dir.to_string_lossy().as_ref()),
        );

        assert_eq!(scoped["total"], 1);
        assert_eq!(scoped["dead_symbols"][0]["name"], "alpha");
        assert_eq!(excluded["total"], 1);
        assert_eq!(excluded["dead_symbols"][0]["name"], "alpha");
        assert_eq!(excluded["skipped_excluded"], 1);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_architecture_filters_generic_python_builtins() {
        let graph = CodeGraph::new();
        let file = "/tmp/browser_use/session.py";

        graph.add_node(UniversalNode {
            id: "append".into(),
            name: "append".into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: file.into(),
                start_line: 10,
                end_line: 12,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            ..Default::default()
        });
        graph.add_node(UniversalNode {
            id: "__init__".into(),
            name: "__init__".into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: file.into(),
                start_line: 20,
                end_line: 25,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            ..Default::default()
        });
        graph.add_node(UniversalNode {
            id: "browser-session".into(),
            name: "BrowserSession".into(),
            node_type: NodeType::Class,
            location: UniversalLocation {
                file_path: file.into(),
                start_line: 30,
                end_line: 60,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            ..Default::default()
        });
        graph.add_node(UniversalNode {
            id: "caller-a".into(),
            name: "make_session".into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: file.into(),
                start_line: 70,
                end_line: 72,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            ..Default::default()
        });
        graph.add_node(UniversalNode {
            id: "caller-b".into(),
            name: "close_session".into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: file.into(),
                start_line: 80,
                end_line: 82,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            ..Default::default()
        });

        graph.add_relationship(UniversalRelationship {
            id: "rel-1".into(),
            source_id: "caller-a".into(),
            target_id: "append".into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });
        graph.add_relationship(UniversalRelationship {
            id: "rel-2".into(),
            source_id: "caller-b".into(),
            target_id: "__init__".into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });
        graph.add_relationship(UniversalRelationship {
            id: "rel-3".into(),
            source_id: "caller-a".into(),
            target_id: "browser-session".into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });
        graph.add_relationship(UniversalRelationship {
            id: "rel-4".into(),
            source_id: "caller-b".into(),
            target_id: "browser-session".into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });

        let result = handle_architecture(&graph, None);
        let names = result["hub_symbols"]
            .as_array()
            .expect("hub symbols")
            .iter()
            .filter_map(|entry| entry["name"].as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"BrowserSession"));
        assert!(!names.contains(&"append"));
        assert!(!names.contains(&"__init__"));
    }

    #[test]
    fn test_coverage_map_uses_test_symbol_overlap_for_probable_matches() {
        let graph = CodeGraph::new();
        let source = "/tmp/repo/traverse/browser/session.py";
        let test = "/tmp/repo/tests/ci/test_cross_origin_click.py";

        graph.add_node(UniversalNode {
            id: "browser-session".into(),
            name: "BrowserSession".into(),
            node_type: NodeType::Class,
            location: UniversalLocation {
                file_path: source.into(),
                start_line: 1,
                end_line: 20,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            ..Default::default()
        });
        graph.add_node(UniversalNode {
            id: "test-browser-session".into(),
            name: "browser_session".into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: test.into(),
                start_line: 1,
                end_line: 5,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            ..Default::default()
        });

        let result = handle_test_coverage_map(&graph, Some("/tmp/repo"));
        assert_eq!(result["covered_files"], 1);
        assert_eq!(result["conservative_covered_files"], 0);
        assert_eq!(result["likely_covered_files"], 1);
        assert_eq!(result["likely_covered"][0], "traverse/browser/session.py");
        assert_eq!(result["coverage_range_percent"]["lower_bound"], 0.0);
        assert_eq!(result["coverage_range_percent"]["upper_bound"], 100.0);
        assert!(result["interpretation"]
            .as_str()
            .unwrap_or("")
            .contains("lower bound"));
    }
}
