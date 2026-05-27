use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use contextro_engines::graph::CodeGraph;
use serde_json::{json, Value};

use super::{is_test_file, resolve_existing_path, strip_base};

/// Circular dependency detection at the file/module import level.
/// Scans `use crate::` and `use super::` statements — not call edges — to avoid
/// false positives from normal cross-module function calls.
pub fn handle_circular_dependencies(graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let nodes = graph.all_nodes();
    let mut all_files: HashSet<String> = HashSet::new();
    for node in &nodes {
        all_files.insert(node.location.file_path.clone());
    }

    let mut rust_files_by_stem: HashMap<String, Vec<String>> = HashMap::new();
    let mut js_relative_imports: HashMap<String, String> = HashMap::new();
    for file_path in &all_files {
        if file_path.ends_with(".rs") {
            if let Some(stem) = Path::new(file_path)
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
            {
                rust_files_by_stem
                    .entry(stem)
                    .or_default()
                    .push(file_path.clone());
            }
        }

        if is_javascript_like_file(file_path) {
            register_js_import_candidates(file_path, &mut js_relative_imports);
        }
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
                        if let Some(candidates) = rust_files_by_stem.get(segment) {
                            if let Some(dep) =
                                candidates.iter().find(|candidate| *candidate != file_path)
                            {
                                file_deps
                                    .entry(file_path.clone())
                                    .or_default()
                                    .insert(dep.clone());
                            }
                        }
                    }
                }
            }

            // TypeScript/JavaScript: import ... from './relative/path'
            if is_javascript_like_file(file_path) && trimmed.starts_with("import ") {
                // Extract the path from: import ... from './foo' or import ... from '../bar'
                if let Some(from_pos) = trimmed.find("from ") {
                    let after_from = trimmed[from_pos + 5..].trim();
                    let path_str = after_from.trim_end_matches(';').trim_matches(['\'', '"']);
                    // Only relative imports can form cycles
                    if path_str.starts_with('.') {
                        let import_key = javascript_import_lookup_key(file_path, path_str);
                        if let Some(dep) = js_relative_imports.get(&import_key) {
                            if dep != file_path {
                                file_deps
                                    .entry(file_path.clone())
                                    .or_default()
                                    .insert(dep.clone());
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
    let nodes = graph.all_nodes();
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

fn is_javascript_like_file(file_path: &str) -> bool {
    file_path.ends_with(".ts")
        || file_path.ends_with(".tsx")
        || file_path.ends_with(".js")
        || file_path.ends_with(".jsx")
}

fn register_js_import_candidates(file_path: &str, imports: &mut HashMap<String, String>) {
    let file_path_buf = PathBuf::from(file_path);
    let Some(parent) = file_path_buf.parent() else {
        return;
    };
    let stem = file_path_buf
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();
    if stem.is_empty() {
        return;
    }

    let dir = parent.to_path_buf();
    let mut candidates = vec![dir.join(&stem)];
    if stem == "index" {
        candidates.push(dir.clone());
    }

    for candidate in candidates {
        let key = normalize_lookup_path(&candidate);
        imports.entry(key).or_insert_with(|| file_path.to_string());
    }
}

fn javascript_import_lookup_key(file_path: &str, import_path: &str) -> String {
    normalize_lookup_path(
        &Path::new(file_path)
            .parent()
            .unwrap_or(Path::new(""))
            .join(import_path),
    )
}

fn normalize_lookup_path(path: &Path) -> String {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized.to_string_lossy().replace('\\', "/")
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
