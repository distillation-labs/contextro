//! Analysis tools: overview, architecture, analyze, focus, dead_code, circular_dependencies, test_coverage_map.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use contextro_engines::graph::CodeGraph;
use serde_json::{json, Value};

mod dead_code;
mod file_relations;

pub use dead_code::handle_dead_code;
pub use file_relations::{handle_circular_dependencies, handle_test_coverage_map};

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
    let nodes = graph.all_nodes();
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

pub fn handle_architecture(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let snapshot = graph.snapshot();
    let nodes = snapshot.nodes();
    let mut scored: Vec<(String, String, usize)> = nodes
        .iter()
        .filter(|n| !is_generic_symbol_name(&n.name))
        .filter(|n| !is_test_file(&n.location.file_path))
        .map(|n| {
            let (in_d, out_d) = snapshot.degree(&n.id);
            (n.name.clone(), n.location.file_path.clone(), in_d + out_d)
        })
        .collect();
    scored.sort_by_key(|b| std::cmp::Reverse(b.2));

    let hubs: Vec<Value> = scored
        .iter()
        .take(limit)
        .map(|(name, file, degree)| {
            let fp = strip_base(file, codebase);
            json!({"name": name, "file": fp, "degree": degree})
        })
        .collect();

    json!({"hub_symbols": hubs, "total_nodes": graph.node_count(), "total_edges": graph.relationship_count(), "limit": limit})
}

pub fn handle_analyze(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let path_filter = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let min_connections = args
        .get("min_connections")
        .and_then(|v| v.as_u64())
        .unwrap_or(6) as usize;
    let top_n = args.get("top_n").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let snapshot = graph.snapshot();
    let mut normalized_paths = HashMap::new();

    // Filter nodes to the requested path prefix if specified
    let nodes: Vec<_> = if path_filter.is_empty() {
        snapshot.nodes().iter().collect()
    } else {
        let abs_filter = match resolve_existing_path(path_filter, codebase) {
            Ok(path) => path,
            Err(error) => return error,
        };
        let is_dir = abs_filter.is_dir();
        snapshot
            .nodes()
            .iter()
            .filter(|n| {
                path_matches_cached(
                    &n.location.file_path,
                    &abs_filter,
                    is_dir,
                    &mut normalized_paths,
                )
            })
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
        let (in_d, out_d) = snapshot.degree(&node.id);
        if in_d + out_d >= min_connections {
            complex_fns.push(json!({"name": node.name, "file": strip_base(&node.location.file_path, codebase), "connections": in_d + out_d}));
        }
    }
    complex_fns.sort_by_key(|v| std::cmp::Reverse(v["connections"].as_u64().unwrap_or(0)));
    complex_fns.truncate(top_n);

    let mut large_files: Vec<Value> = file_sizes
        .iter()
        .filter(|(_, count)| **count > 10)
        .map(|(file, count)| json!({"file": strip_base(file, codebase), "symbols": count}))
        .collect();
    large_files.sort_by_key(|v| std::cmp::Reverse(v["symbols"].as_u64().unwrap_or(0)));

    json!({
        "path": if path_filter.is_empty() { Value::Null } else { json!(path_filter) },
        "high_connectivity_symbols": complex_fns,
        "large_files": large_files,
        "total_symbols": nodes.len(),
        "min_connections": min_connections,
        "top_n": top_n
    })
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

    let snapshot = graph.snapshot();
    let nodes = snapshot.nodes();
    let mut normalized_paths = HashMap::new();

    // Directory: list top symbols grouped by file
    if abs_path.is_dir() {
        let mut by_file: std::collections::BTreeMap<String, Vec<Value>> =
            std::collections::BTreeMap::new();
        for n in nodes.iter().filter(|n| {
            path_matches_cached(
                &n.location.file_path,
                &abs_path,
                true,
                &mut normalized_paths,
            )
        }) {
            let (in_d, out_d) = snapshot.degree(&n.id);
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
            let (in_d, out_d) = snapshot.degree(&n.id);
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

fn path_matches_cached(
    file_path: &str,
    target_path: &Path,
    target_is_dir: bool,
    normalized_paths: &mut HashMap<String, PathBuf>,
) -> bool {
    let normalized_file = normalized_paths
        .entry(file_path.to_string())
        .or_insert_with(|| {
            std::fs::canonicalize(file_path).unwrap_or_else(|_| PathBuf::from(file_path))
        });
    if target_is_dir {
        *normalized_file == target_path || normalized_file.starts_with(target_path)
    } else {
        *normalized_file == target_path
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

#[cfg(test)]
mod tests;
