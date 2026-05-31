//! Graph-based tools: find_callers, find_callees, test_for, explain, impact.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::{Arc, OnceLock};

use contextro_engines::graph::CodeGraph;
use parking_lot::RwLock;
use serde_json::{json, Value};

use crate::analysis::{
    file_relations::{coverage_tokens, file_stem_stripped, has_probable_test_signal},
    is_test_file, strip_base,
};

/// Resolve the preferred `symbol_name` plus backward-compatible aliases.
fn get_symbol_name(args: &Value) -> &str {
    args.get("symbol_name")
        .or_else(|| args.get("name"))
        .or_else(|| args.get("symbol"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

pub fn handle_find_callers(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let name = get_symbol_name(args);
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    if name.is_empty() {
        return json!({"error": "Missing required parameter: symbol_name"});
    }

    let matches = resolve_symbol(name, graph);
    if matches.is_empty() {
        return json!({"error": format!("Symbol '{}' not found.", name)});
    }

    let mut callers = Vec::new();
    for node in &matches {
        for caller in graph.get_callers(&node.id) {
            let fp = relativize(&caller.location.file_path, codebase);
            callers.push(format!(
                "{} ({}:{})",
                caller.name, fp, caller.location.start_line
            ));
            if callers.len() >= limit {
                break;
            }
        }
        if callers.len() >= limit {
            break;
        }
    }

    let mut result =
        json!({"symbol": name, "callers": callers, "total": callers.len(), "limit": limit});
    if callers.is_empty() {
        let is_type = matches.iter().any(|n| {
            matches!(
                n.node_type,
                contextro_core::NodeType::Class
                    | contextro_core::NodeType::Interface
                    | contextro_core::NodeType::Enum
            )
        });
        if is_type {
            result["hint"] = json!(
                "This is a type (struct/class/enum) — types have no call-graph edges. \
                 Try querying a method or constructor: find_callers('new') or search() for usage."
            );
        }
    }
    result
}

pub fn handle_find_callees(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let name = get_symbol_name(args);
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    if name.is_empty() {
        return json!({"error": "Missing required parameter: symbol_name"});
    }

    let matches = resolve_symbol(name, graph);
    if matches.is_empty() {
        return json!({"error": format!("Symbol '{}' not found.", name)});
    }

    let mut callees = Vec::new();
    for node in &matches {
        for callee in graph.get_callees(&node.id) {
            let fp = relativize(&callee.location.file_path, codebase);
            callees.push(format!(
                "{} ({}:{})",
                callee.name, fp, callee.location.start_line
            ));
            if callees.len() >= limit {
                break;
            }
        }
        if callees.len() >= limit {
            break;
        }
    }

    let mut result =
        json!({"symbol": name, "callees": callees, "total": callees.len(), "limit": limit});
    if callees.is_empty() {
        let is_type = matches.iter().any(|n| {
            matches!(
                n.node_type,
                contextro_core::NodeType::Class
                    | contextro_core::NodeType::Interface
                    | contextro_core::NodeType::Enum
            )
        });
        if is_type {
            result["hint"] = json!(
                "This is a type (struct/class/enum) — types have no call-graph edges. \
                 Try querying its methods directly by name."
            );
        }
    }
    result
}

pub fn handle_test_for(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    handle_test_for_with_epoch(args, graph, codebase, graph.mutation_epoch())
}

pub fn handle_test_for_with_epoch(
    args: &Value,
    graph: &CodeGraph,
    codebase: Option<&str>,
    graph_epoch: u64,
) -> Value {
    let name = get_symbol_name(args);
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
    if name.is_empty() {
        return json!({"error": "Missing required parameter: symbol_name"});
    }

    let matches = resolve_symbol(name, graph);
    if matches.is_empty() {
        return json!({"error": format!("Symbol '{}' not found.", name)});
    }
    let source_tokens = collect_source_tokens(&matches, codebase);
    let cached = cached_test_for_graph_data(graph, codebase, graph_epoch);
    let source_stems: HashSet<String> = matches
        .iter()
        .map(|node| file_stem_stripped(&node.location.file_path))
        .filter(|stem| !stem.is_empty())
        .collect();

    let mut candidates: HashMap<String, TestCandidate> = HashMap::new();

    for node in &matches {
        if file_has_inline_tests(&node.location.file_path) {
            add_test_signal(
                &mut candidates,
                &node.location.file_path,
                90,
                "inline_test",
                None,
                None,
            );
        }
    }

    queue_test_callers(&matches, graph, &mut candidates);

    for (test_file, tokens) in cached.test_file_tokens.iter() {
        if source_stems.contains(&file_stem_stripped(test_file)) {
            add_test_signal(&mut candidates, test_file, 60, "exact_stem", None, None);
        }
        if has_probable_test_signal(&source_tokens, tokens, &cached.source_token_frequency) {
            add_test_signal(&mut candidates, test_file, 30, "token_overlap", None, None);
        }
    }

    let candidate_total = candidates.len();
    let mut tests: Vec<Value> = candidates
        .into_iter()
        .map(|(file, candidate)| candidate.to_json(&file, codebase))
        .collect();
    tests.sort_by(|left, right| {
        right["score"]
            .as_u64()
            .cmp(&left["score"].as_u64())
            .then_with(|| left["file"].as_str().cmp(&right["file"].as_str()))
    });
    if tests.len() > limit {
        tests.truncate(limit);
    }

    let definitions: Vec<Value> = matches
        .iter()
        .map(|node| {
            json!({
                "name": node.name,
                "type": node.node_type.to_string(),
                "file": relativize(&node.location.file_path, codebase),
                "line": node.location.start_line,
            })
        })
        .collect();

    let mut result = json!({
        "symbol": name,
        "definitions": definitions,
        "tests": tests,
        "total": tests.len(),
        "candidate_total": candidate_total,
        "limit": limit,
    });
    if candidate_total == 0 {
        result["hint"] = json!(
            "No direct or heuristic test matches found. Try test_coverage_map() for repo-wide gaps."
        );
    }
    result
}

pub fn handle_explain(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let name = get_symbol_name(args);
    if name.is_empty() {
        return json!({"error": "Missing required parameter: symbol_name"});
    }

    let matches = resolve_symbol(name, graph);
    if matches.is_empty() {
        return json!({"error": format!("Symbol '{}' not found.", name)});
    }

    let node = &matches[0];
    let all_callers = graph.get_callers(&node.id);
    let all_callees = graph.get_callees(&node.id);
    let callers: Vec<String> = all_callers
        .iter()
        .take(10)
        .map(|c| {
            format!(
                "{} ({}:{})",
                c.name,
                relativize(&c.location.file_path, codebase),
                c.location.start_line
            )
        })
        .collect();
    let callees: Vec<String> = all_callees
        .iter()
        .take(10)
        .map(|c| {
            // #4: Type-qualified name if parent is available
            let display = if let Some(ref parent) = c.parent {
                format!("{}.{}", parent, c.name)
            } else {
                c.name.clone()
            };
            format!(
                "{} ({}:{})",
                display,
                relativize(&c.location.file_path, codebase),
                c.location.start_line
            )
        })
        .collect();
    let summary = build_explanation_summary(node, all_callers.len(), all_callees.len(), codebase);

    json!({
        "name": node.name,
        "type": node.node_type.to_string(),
        "file": relativize(&node.location.file_path, codebase),
        "line": node.location.start_line,
        "language": node.language,
        "docstring": node.docstring,
        "summary": summary,
        "callers_count": all_callers.len(),
        "callees_count": all_callees.len(),
        "callers": callers,
        "callees": callees,
    })
}

pub fn handle_impact(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    const DEFAULT_IMPACT_DEPTH: usize = 5;
    let name = get_symbol_name(args);
    let requested_depth = args.get("max_depth").and_then(|v| v.as_u64());
    let max_depth = requested_depth.unwrap_or(DEFAULT_IMPACT_DEPTH as u64) as usize;

    if name.is_empty() {
        return json!({"error": "Missing required parameter: symbol_name"});
    }

    let matches = resolve_symbol(name, graph);
    if matches.is_empty() {
        return json!({"error": format!("Symbol '{}' not found.", name)});
    }

    // BFS transitive callers
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    let mut impacted = Vec::new();

    for node in &matches {
        queue.push_back((node.id.clone(), 0));
        visited.insert(node.id.clone());
    }

    while let Some((node_id, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        for caller in graph.get_callers(&node_id) {
            if visited.insert(caller.id.clone()) {
                let fp = relativize(&caller.location.file_path, codebase);
                impacted.push(json!({
                    "name": caller.name,
                    "file": fp,
                    "line": caller.location.start_line,
                    "depth": depth + 1,
                }));
                queue.push_back((caller.id.clone(), depth + 1));
            }
        }
    }

    let mut result = json!({
        "symbol": name,
        "max_depth": max_depth,
        "default_depth": DEFAULT_IMPACT_DEPTH,
        "impacted": impacted,
        "total": impacted.len(),
        "total_impacted": impacted.len(),
    });

    if let Some(explicit_depth) = requested_depth {
        if explicit_depth as usize != DEFAULT_IMPACT_DEPTH {
            result["depth_hint"] = json!(format!(
                "Explicit max_depth={} overrides the default depth of {}. Smaller depths intentionally return a narrower impact set.",
                explicit_depth,
                DEFAULT_IMPACT_DEPTH
            ));
        }
    }

    // Hint for entry points: 0 transitive callers means nothing depends on this symbol,
    // which is expected for top-level entry points (main, CLI handlers, etc.)
    if impacted.is_empty() {
        let (in_degree, _) = graph.get_node_degree(&matches[0].id);
        if in_degree == 0 {
            result["hint"] = json!(
                "0 callers found — this symbol is a root entry point (nothing calls it in the parsed AST). \
                 It is safe to change its signature, but check external callers (CLI, tests, MCP handlers) manually."
            );
        }
    }

    result
}

fn relativize(filepath: &str, codebase: Option<&str>) -> String {
    match codebase {
        Some(base) => Path::new(filepath)
            .strip_prefix(base)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| filepath.to_string()),
        None => filepath.to_string(),
    }
}

/// Resolve a symbol name: exact match first, fall back to fuzzy.
/// Ranks candidates by call frequency so the most-connected symbol wins on name collision.
pub(crate) fn resolve_symbol(name: &str, graph: &CodeGraph) -> Vec<contextro_core::UniversalNode> {
    let exact = graph.find_nodes_by_name(name, true);
    if !exact.is_empty() {
        let mut ranked = exact;
        ranked.sort_by_key(|n| {
            let (in_d, out_d) = graph.get_node_degree(&n.id);
            std::cmp::Reverse(in_d + out_d)
        });
        return ranked;
    }
    let mut fuzzy = graph.find_nodes_by_name(name, false);
    fuzzy.sort_by_key(|n| {
        let (in_d, out_d) = graph.get_node_degree(&n.id);
        std::cmp::Reverse(in_d + out_d)
    });
    fuzzy.into_iter().take(5).collect()
}

fn build_explanation_summary(
    node: &contextro_core::UniversalNode,
    callers_count: usize,
    callees_count: usize,
    codebase: Option<&str>,
) -> String {
    let location = format!(
        "{}:{}",
        relativize(&node.location.file_path, codebase),
        node.location.start_line
    );
    let doc = node
        .docstring
        .as_deref()
        .map(str::trim)
        .filter(|doc| !doc.is_empty())
        .map(|doc| format!(" {doc}"))
        .unwrap_or_default();
    format!(
        "{} is a {} defined at {}. It currently has {} caller(s) and {} callee(s).{}",
        node.name, node.node_type, location, callers_count, callees_count, doc
    )
}

#[derive(Default)]
struct TestCandidate {
    score: u64,
    signals: BTreeSet<&'static str>,
    callers: BTreeSet<String>,
    line: Option<u32>,
}

#[derive(Clone, Default)]
struct CachedTestForGraphData {
    graph_ptr: usize,
    graph_epoch: u64,
    codebase: Option<String>,
    test_file_tokens: Arc<HashMap<String, HashSet<String>>>,
    source_token_frequency: Arc<HashMap<String, usize>>,
}

fn test_for_graph_cache() -> &'static RwLock<CachedTestForGraphData> {
    static CACHE: OnceLock<RwLock<CachedTestForGraphData>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(CachedTestForGraphData::default()))
}

fn cached_test_for_graph_data(
    graph: &CodeGraph,
    codebase: Option<&str>,
    graph_epoch: u64,
) -> CachedTestForGraphData {
    let graph_ptr = graph as *const CodeGraph as usize;
    let codebase_owned = codebase.map(str::to_owned);
    {
        let cache = test_for_graph_cache().read();
        if cache.graph_ptr == graph_ptr
            && cache.graph_epoch == graph_epoch
            && cache.codebase == codebase_owned
        {
            return cache.clone();
        }
    }

    let refreshed = CachedTestForGraphData {
        graph_ptr,
        graph_epoch,
        codebase: codebase_owned,
        test_file_tokens: Arc::new(collect_test_file_tokens(graph, codebase)),
        source_token_frequency: Arc::new(build_source_token_frequency(graph, codebase)),
    };
    *test_for_graph_cache().write() = refreshed.clone();
    refreshed
}

impl TestCandidate {
    fn to_json(self, file: &str, codebase: Option<&str>) -> Value {
        let mut value = json!({
            "file": strip_base(file, codebase),
            "score": self.score,
            "signals": self.signals.into_iter().collect::<Vec<_>>(),
        });
        if !self.callers.is_empty() {
            value["callers"] = json!(self.callers.into_iter().collect::<Vec<_>>());
        }
        if let Some(line) = self.line {
            value["line"] = json!(line);
        }
        value
    }
}

fn add_test_signal(
    candidates: &mut HashMap<String, TestCandidate>,
    file: &str,
    score: u64,
    signal: &'static str,
    caller_name: Option<&str>,
    line: Option<u32>,
) {
    let entry = candidates.entry(file.to_string()).or_default();
    entry.score += score;
    entry.signals.insert(signal);
    if let Some(name) = caller_name {
        entry.callers.insert(name.to_string());
    }
    if let Some(line) = line {
        entry.line = Some(entry.line.map_or(line, |current| current.min(line)));
    }
}

fn queue_test_callers(
    matches: &[contextro_core::UniversalNode],
    graph: &CodeGraph,
    candidates: &mut HashMap<String, TestCandidate>,
) {
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    for node in matches {
        visited.insert(node.id.clone());
        queue.push_back((node.id.clone(), 0));
    }

    while let Some((node_id, depth)) = queue.pop_front() {
        if depth >= 2 {
            continue;
        }
        for caller in graph.get_callers(&node_id) {
            if !visited.insert(caller.id.clone()) {
                continue;
            }

            let next_depth = depth + 1;
            if is_test_file(&caller.location.file_path) {
                let (signal, score) = if next_depth == 1 {
                    ("direct_call", 100)
                } else {
                    ("transitive_call", 80)
                };
                add_test_signal(
                    candidates,
                    &caller.location.file_path,
                    score,
                    signal,
                    Some(&caller.name),
                    Some(caller.location.start_line),
                );
            }

            if next_depth < 2 {
                queue.push_back((caller.id.clone(), next_depth));
            }
        }
    }
}

fn collect_source_tokens(
    matches: &[contextro_core::UniversalNode],
    codebase: Option<&str>,
) -> HashSet<String> {
    let mut tokens = HashSet::new();
    for node in matches {
        tokens.extend(coverage_tokens(&strip_base(
            &node.location.file_path,
            codebase,
        )));
        tokens.extend(coverage_tokens(&node.name));
        if let Some(parent) = &node.parent {
            tokens.extend(coverage_tokens(parent));
        }
    }
    tokens
}

fn collect_test_file_tokens(
    graph: &CodeGraph,
    codebase: Option<&str>,
) -> HashMap<String, HashSet<String>> {
    let mut test_tokens = HashMap::new();
    for node in graph.all_nodes() {
        if !is_test_file(&node.location.file_path) {
            continue;
        }
        let entry = test_tokens
            .entry(node.location.file_path.clone())
            .or_insert_with(|| coverage_tokens(&strip_base(&node.location.file_path, codebase)));
        entry.extend(coverage_tokens(&node.name));
        if let Some(parent) = &node.parent {
            entry.extend(coverage_tokens(parent));
        }
    }
    test_tokens
}

fn build_source_token_frequency(
    graph: &CodeGraph,
    codebase: Option<&str>,
) -> HashMap<String, usize> {
    let mut file_tokens = HashMap::<String, HashSet<String>>::new();
    for node in graph.all_nodes() {
        if is_test_file(&node.location.file_path) {
            continue;
        }
        file_tokens
            .entry(node.location.file_path.clone())
            .or_insert_with(|| coverage_tokens(&strip_base(&node.location.file_path, codebase)));
    }

    let mut frequency = HashMap::new();
    for tokens in file_tokens.into_values() {
        for token in tokens {
            *frequency.entry(token).or_insert(0) += 1;
        }
    }
    frequency
}

fn file_has_inline_tests(file_path: &str) -> bool {
    std::fs::read_to_string(file_path)
        .map(|content| {
            content.contains("#[cfg(test)]")
                || content.contains("#[test]")
                || content.contains("describe(")
                || content.contains("test(")
                || content.contains("it(")
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests;
