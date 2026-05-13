//! Analysis tools: overview, architecture, analyze, focus, dead_code, circular_dependencies, test_coverage_map.

use std::collections::{HashMap, HashSet};
use std::path::Path;

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

pub fn handle_overview(
    graph: &CodeGraph,
    codebase: Option<&str>,
    total_chunks: usize,
    vector_chunks: usize,
) -> Value {
    let node_count = graph.node_count();
    let rel_count = graph.relationship_count();
    json!({
        "codebase_path": codebase,
        "total_symbols": node_count,
        "total_relationships": rel_count,
        "total_chunks": total_chunks,
        "vector_chunks": vector_chunks,
    })
}

pub fn handle_architecture(graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let nodes = graph.find_nodes_by_name("", false);
    let mut scored: Vec<(String, String, usize)> = nodes
        .iter()
        .filter(|n| !GENERIC_NAMES.contains(&n.name.as_str()))
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
        let abs_filter = if std::path::Path::new(path_filter).is_absolute() {
            path_filter.to_string()
        } else {
            codebase
                .map(|b| format!("{}/{}", b, path_filter))
                .unwrap_or_else(|| path_filter.to_string())
        };
        all_nodes
            .into_iter()
            .filter(|n| n.location.file_path.starts_with(&abs_filter))
            .collect()
    };

    let mut complex_fns: Vec<Value> = Vec::new();
    let mut file_sizes: HashMap<String, usize> = HashMap::new();

    for node in &nodes {
        *file_sizes
            .entry(node.location.file_path.clone())
            .or_default() += 1;
        if GENERIC_NAMES.contains(&node.name.as_str()) || is_test_file(&node.location.file_path) {
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

    json!({"high_connectivity_symbols": complex_fns, "large_files": large_files, "total_symbols": nodes.len()})
}

/// Low-token context slice for a single file.
pub fn handle_focus(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return json!({"error": "Missing required parameter: path"});
    }

    let abs_path = if Path::new(path).is_absolute() {
        path.to_string()
    } else {
        codebase
            .map(|b| format!("{}/{}", b, path))
            .unwrap_or_else(|| path.to_string())
    };

    let nodes = graph.find_nodes_by_name("", false);

    // Directory: list top symbols grouped by file
    if Path::new(&abs_path).is_dir() {
        let mut by_file: std::collections::BTreeMap<String, Vec<Value>> =
            std::collections::BTreeMap::new();
        for n in nodes
            .iter()
            .filter(|n| n.location.file_path.starts_with(&abs_path))
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
            "path": strip_base(&abs_path, codebase),
            "is_directory": true,
            "files": files,
            "total_symbols": total_symbols,
        });
    }

    // Single file
    let file_symbols: Vec<Value> = nodes.iter()
        .filter(|n| n.location.file_path == abs_path || n.location.file_path.ends_with(path))
        .map(|n| {
            let (in_d, out_d) = graph.get_node_degree(&n.id);
            json!({"name": n.name, "type": n.node_type.to_string(), "line": n.location.start_line, "callers": in_d, "callees": out_d})
        })
        .collect();

    let preview = std::fs::read_to_string(&abs_path)
        .map(|s| s.lines().take(5).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default();

    json!({
        "file": strip_base(&abs_path, codebase),
        "symbols": file_symbols,
        "total_symbols": file_symbols.len(),
        "preview": preview,
    })
}

/// Dead code analysis: find symbols with zero callers that aren't entry points.
pub fn handle_dead_code(graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let nodes = graph.find_nodes_by_name("", false);
    let mut dead: Vec<Value> = Vec::new();

    for node in &nodes {
        // Skip classes and variables — focus on functions/methods
        if node.node_type != NodeType::Function {
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
            let is_noise = GENERIC_NAMES.contains(&node.name.as_str())
                || GENERIC_NAMES.contains(&name_lower.as_str());
            if !is_entry && !is_noise {
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
    dead.truncate(50);

    json!({"dead_symbols": dead, "total": dead.len(), "note": "Symbols with zero callers that aren't entry points"})
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
                let after_prefix = if trimmed.starts_with("use crate::") {
                    Some(&trimmed[11..])
                } else if trimmed.starts_with("use super::") {
                    Some(&trimmed[11..])
                } else {
                    None
                };
                if let Some(after) = after_prefix {
                    let segment = after
                        .split(|c: char| c == ':' || c == ';' || c == ' ' || c == '{' || c == ',')
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
                        .trim_start_matches(|c| c == '\'' || c == '"')
                        .trim_end_matches(|c| c == '\'' || c == '"' || c == ';');
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

    for node in &nodes {
        let fp = &node.location.file_path;
        if is_test_file(fp) {
            test_files.insert(fp.clone());
        } else {
            source_files.insert(fp.clone());
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

    let mut covered: Vec<String> = Vec::new();
    let mut uncovered: Vec<String> = Vec::new();

    for src in &source_files {
        let src_stem = file_stem_stripped(src);

        let has_test = inline_tested.contains(src)
            || test_files.iter().any(|t| {
                let t_stem = file_stem_stripped(t);
                t_stem == src_stem
                    || t_stem == format!("test_{}", src_stem)
                    || t_stem == format!("{}_test", src_stem)
            });

        if has_test {
            covered.push(strip_base(src, codebase));
        } else {
            uncovered.push(strip_base(src, codebase));
        }
    }

    let coverage_pct = if source_files.is_empty() {
        0.0
    } else {
        covered.len() as f64 / source_files.len() as f64 * 100.0
    };

    json!({
        "coverage_percent": (coverage_pct * 10.0).round() / 10.0,
        "covered_files": covered.len(),
        "uncovered_files": uncovered.len(),
        "test_files": test_files.len() + inline_tested.len(),
        "uncovered": uncovered.into_iter().take(20).collect::<Vec<_>>(),
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

fn is_test_file(fp: &str) -> bool {
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

fn strip_base(file: &str, codebase: Option<&str>) -> String {
    codebase
        .and_then(|b| Path::new(file).strip_prefix(b).ok())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| file.to_string())
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
