//! Artifact tools: audit, docs_bundle, sidecar_export, skill_prompt, introspect, status, health, refactor_check, completion_check.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::analysis::{is_generic_symbol_name, is_test_file, strip_base};
use crate::tool_manifest::{find_tool_doc, tool_docs, ToolDoc};
use contextro_engines::graph::CodeGraph;
use serde_json::{json, Value};

fn tool_doc_summary(doc: &ToolDoc) -> Value {
    json!({"name": doc.name, "tool": doc.name, "description": doc.description})
}

fn tool_doc_detail(doc: &ToolDoc) -> Value {
    json!({
        "name": doc.name,
        "tool": doc.name,
        "description": doc.description,
        "parameters": doc.parameters,
        "example": doc.example,
    })
}

fn tool_doc_haystack(doc: &ToolDoc) -> String {
    format!(
        "{} {} {} {}",
        doc.name,
        doc.description,
        doc.parameters.join(" "),
        doc.example
    )
    .to_lowercase()
}

fn sort_counts(counts: HashMap<String, usize>) -> Vec<(String, usize)> {
    let mut pairs: Vec<(String, usize)> = counts.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    pairs
}

const AUDIT_CONNECTION_THRESHOLD: usize = 10;
const AUDIT_FILE_SYMBOL_THRESHOLD: usize = 30;
const AUDIT_EVIDENCE_LIMIT: usize = 3;

fn audit_quality_score(
    recommendation_count: usize,
    max_connection_overage: usize,
    max_file_overage: usize,
) -> usize {
    if recommendation_count == 0 {
        95
    } else {
        85usize
            .saturating_sub(recommendation_count * 5)
            .saturating_sub(max_connection_overage / AUDIT_CONNECTION_THRESHOLD)
            .saturating_sub(max_file_overage / AUDIT_FILE_SYMBOL_THRESHOLD)
    }
}

fn is_audit_noise_file(file_path: &str) -> bool {
    is_test_file(file_path)
        || file_path.starts_with("tests/")
        || file_path.starts_with("test/")
        || file_path.starts_with("__tests__/")
        || file_path.starts_with("e2e/")
        || file_path.starts_with("spec/")
}

fn audit_command_arg(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn push_count_section(
    markdown: &mut String,
    title: &str,
    items: &[(String, usize)],
    value_label: &str,
) {
    markdown.push_str(&format!("## {}\n\n", title));
    if items.is_empty() {
        markdown.push_str("_No data available._\n\n");
        return;
    }
    for (name, count) in items.iter().take(10) {
        markdown.push_str(&format!("- `{}` — {} {}\n", name, count, value_label));
    }
    markdown.push('\n');
}

fn sidecar_target_matches(
    file_path: &str,
    target_abs: &Path,
    target_rel: &str,
    target_is_dir: bool,
    codebase: Option<&str>,
) -> bool {
    if target_rel.is_empty() {
        return true;
    }

    let normalized_file =
        std::fs::canonicalize(file_path).unwrap_or_else(|_| PathBuf::from(file_path));
    if target_is_dir {
        if normalized_file == target_abs || normalized_file.starts_with(target_abs) {
            return true;
        }
    } else if normalized_file == target_abs {
        return true;
    }

    let relative_file = strip_base(file_path, codebase);
    let normalized_target_rel = target_rel.trim_matches('/').replace('\\', "/");
    let normalized_relative = relative_file.replace('\\', "/");
    let normalized_original = file_path.replace('\\', "/");
    if target_is_dir {
        normalized_relative == normalized_target_rel
            || normalized_relative.starts_with(&format!("{normalized_target_rel}/"))
            || normalized_original.contains(&format!("/{normalized_target_rel}/"))
            || normalized_original.ends_with(&format!("/{normalized_target_rel}"))
    } else {
        normalized_relative == normalized_target_rel
            || normalized_original.ends_with(&format!("/{normalized_target_rel}"))
    }
}

/// Generate an audit report with recommendations.
pub fn handle_audit(graph: &CodeGraph, codebase: Option<&str>) -> Value {
    let snapshot = graph.snapshot();
    let degree_by_id: HashMap<String, (usize, usize)> = snapshot
        .nodes()
        .iter()
        .map(|node| (node.id.clone(), snapshot.degree(&node.id)))
        .collect();
    let all_nodes = snapshot.into_nodes();
    let total_symbols = all_nodes.len();
    let nodes: Vec<_> = all_nodes
        .into_iter()
        .filter(|node| !is_generic_symbol_name(&node.name))
        .filter(|node| !is_audit_noise_file(&node.location.file_path))
        .collect();
    let mut recommendations: Vec<Value> = Vec::new();

    // Check for high-complexity symbols and keep only the top offenders.
    let mut high_conn: Vec<_> = Vec::new();
    for node in &nodes {
        let (in_d, out_d) = degree_by_id.get(&node.id).copied().unwrap_or((0, 0));
        let connections = in_d + out_d;
        if connections > AUDIT_CONNECTION_THRESHOLD {
            high_conn.push((
                node.name.clone(),
                strip_base(&node.location.file_path, codebase),
                connections,
            ));
        }
    }
    high_conn.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    if !high_conn.is_empty() {
        let affected_count = high_conn.len();
        recommendations.push(json!({
            "severity": "medium",
            "category": "complexity",
            "message": format!(
                "{} symbols have >{} connections; inspect the top offenders below",
                affected_count,
                AUDIT_CONNECTION_THRESHOLD
            ),
            "threshold": AUDIT_CONNECTION_THRESHOLD,
            "affected_count": affected_count,
            "evidence": high_conn
                .iter()
                .take(AUDIT_EVIDENCE_LIMIT)
                .map(|(symbol, file, connections)| {
                    json!({
                        "symbol": symbol,
                        "file": file,
                        "connections": connections,
                        "follow_up": [
                            format!("explain({{\"symbol_name\":{}}})", audit_command_arg(symbol)),
                            format!(
                                "impact({{\"symbol_name\":{},\"max_depth\":3}})",
                                audit_command_arg(symbol)
                            ),
                        ]
                    })
                })
                .collect::<Vec<_>>(),
        }));
    }

    // Check file concentration and surface the biggest files first.
    let mut file_counts: HashMap<String, usize> = HashMap::new();
    for node in nodes {
        *file_counts
            .entry(node.location.file_path.clone())
            .or_default() += 1;
    }
    let mut large_files: Vec<_> = file_counts
        .into_iter()
        .filter(|(_, count)| *count > AUDIT_FILE_SYMBOL_THRESHOLD)
        .collect();
    large_files.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    if !large_files.is_empty() {
        let affected_count = large_files.len();
        recommendations.push(json!({
            "severity": "low",
            "category": "structure",
            "message": format!(
                "{} files have >{} symbols; inspect the largest files below",
                affected_count,
                AUDIT_FILE_SYMBOL_THRESHOLD
            ),
            "threshold": AUDIT_FILE_SYMBOL_THRESHOLD,
            "affected_count": affected_count,
            "evidence": large_files
                .iter()
                .take(AUDIT_EVIDENCE_LIMIT)
                .map(|(file, symbols)| {
                    let file = strip_base(file, codebase);
                    json!({
                        "file": file,
                        "symbols": symbols,
                        "follow_up": [
                            format!("analyze({{\"path\":{}}})", audit_command_arg(&file)),
                            format!("focus({{\"path\":{}}})", audit_command_arg(&file)),
                        ]
                    })
                })
                .collect::<Vec<_>>(),
        }));
    }

    let max_connection_overage = high_conn
        .first()
        .map(|(_, _, connections)| connections.saturating_sub(AUDIT_CONNECTION_THRESHOLD))
        .unwrap_or(0);
    let max_file_overage = large_files
        .first()
        .map(|(_, symbols)| symbols.saturating_sub(AUDIT_FILE_SYMBOL_THRESHOLD))
        .unwrap_or(0);
    let quality_score = audit_quality_score(
        recommendations.len(),
        max_connection_overage,
        max_file_overage,
    );

    json!({
        "status": "complete",
        "quality_score": quality_score,
        "total_symbols": total_symbols,
        "recommendations": recommendations,
    })
}

/// Generate a docs bundle.
pub fn handle_docs_bundle(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    if graph.node_count() == 0 {
        return json!({
            "error": "No indexed graph loaded. Run index(path) before docs_bundle.",
            "hint": "Call index({\"path\":\"/path/to/repo\"}) first so Contextro can build the graph used by docs_bundle."
        });
    }

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

    let snapshot = graph.snapshot();
    let nodes = snapshot.nodes();
    let mut file_counts: HashMap<String, usize> = HashMap::new();
    let mut language_counts: HashMap<String, usize> = HashMap::new();
    let mut type_counts: HashMap<String, usize> = HashMap::new();
    let mut directory_counts: HashMap<String, usize> = HashMap::new();

    for node in nodes {
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

    let total_files = file_counts.len();
    let top_languages = sort_counts(language_counts);
    let top_symbol_types = sort_counts(type_counts);
    let top_files = sort_counts(file_counts)
        .into_iter()
        .map(|(file, count)| (strip_base(&file, codebase), count))
        .collect::<Vec<_>>();
    let top_directories = sort_counts(directory_counts)
        .into_iter()
        .map(|(directory, count)| (strip_base(&directory, codebase), count))
        .collect::<Vec<_>>();

    let mut scored: Vec<_> = nodes
        .iter()
        .filter(|n| !is_generic_symbol_name(&n.name))
        .filter(|n| !is_test_file(&n.location.file_path))
        .map(|n| {
            let (i, o) = snapshot.degree(&n.id);
            (
                n.name.clone(),
                strip_base(&n.location.file_path, codebase),
                i + o,
            )
        })
        .collect();
    scored.sort_by_key(|b| std::cmp::Reverse(b.2));

    let mut arch = String::from("# Architecture\n\n");
    arch.push_str("## Hub Symbols\n\n");
    if scored.is_empty() {
        arch.push_str("_No hub symbols found._\n");
    } else {
        for (name, file, degree) in scored.iter().take(10) {
            arch.push_str(&format!(
                "- **{}** (`{}`) — {} connections\n",
                name, file, degree
            ));
        }
    }
    arch.push('\n');
    push_count_section(&mut arch, "Top Directories", &top_directories, "symbols");
    std::fs::write(format!("{}/architecture.md", target), &arch).ok();

    let mut overview = String::from("# Overview\n\n");
    overview.push_str("## Summary\n\n");
    if let Some(codebase) = codebase {
        overview.push_str(&format!("- Codebase: `{}`\n", codebase));
    }
    overview.push_str(&format!(
        "- Total symbols: {}\n- Total relationships: {}\n- Total files: {}\n\n",
        graph.node_count(),
        graph.relationship_count(),
        total_files
    ));
    push_count_section(&mut overview, "Languages", &top_languages, "symbols");
    push_count_section(&mut overview, "Symbol Types", &top_symbol_types, "nodes");
    push_count_section(
        &mut overview,
        "Top Directories",
        &top_directories,
        "symbols",
    );
    push_count_section(&mut overview, "Top Files", &top_files, "symbols");

    overview.push_str("## Hub Symbols\n\n");
    if scored.is_empty() {
        overview.push_str("_No hub symbols found._\n");
    } else {
        for (name, file, degree) in scored.iter().take(10) {
            overview.push_str(&format!(
                "- **{}** (`{}`) — {} connections\n",
                name, file, degree
            ));
        }
    }
    overview.push('\n');
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
    let target_abs = std::fs::canonicalize(&target).unwrap_or_else(|_| PathBuf::from(&target));
    let target_rel = if path == "." || path.is_empty() {
        String::new()
    } else if Path::new(path).is_absolute() {
        strip_base(path, codebase)
    } else {
        path.trim_start_matches("./")
            .trim_end_matches('/')
            .to_string()
    };
    let target_is_dir = Path::new(&target).is_dir();
    if !Path::new(&target).exists() {
        return json!({"error": format!("Path not found: {}", path)});
    }

    // Resolve output directory
    let out_base = if Path::new(output_dir).is_absolute() {
        output_dir.to_string()
    } else {
        format!("{}/{}", base, output_dir)
    };
    std::fs::create_dir_all(&out_base).ok();

    let snapshot = graph.snapshot();
    let nodes = snapshot.nodes();
    let mut files_written = 0;
    let mut matches_by_file: HashMap<String, bool> = HashMap::new();

    // Group symbols by file
    let mut by_file: HashMap<String, Vec<&_>> = HashMap::new();
    for node in nodes {
        let matches_target = *matches_by_file
            .entry(node.location.file_path.clone())
            .or_insert_with(|| {
                sidecar_target_matches(
                    &node.location.file_path,
                    &target_abs,
                    &target_rel,
                    target_is_dir,
                    codebase,
                )
            });
        if matches_target {
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
            let (in_d, out_d) = snapshot.degree(&sym.id);
            content.push_str(&format!(
                "- `{}` ({}) L{} — {} callers, {} callees\n",
                sym.name, sym.node_type, sym.location.start_line, in_d, out_d
            ));
        }
        if std::fs::write(&sidecar_path, &content).is_ok() {
            files_written += 1;
        }
    }

    if files_written == 0 {
        return json!({
            "error": format!("No indexed files matched path: {}", path),
            "path": path,
            "output_dir": out_base,
            "hint": "Pass a file or source subtree from the indexed codebase, not the output directory."
        });
    }

    json!({"status": "exported", "sidecars": files_written, "path": path, "output_dir": out_base})
}

/// Print the agent bootstrap block.
pub fn handle_skill_prompt() -> Value {
    let core_tools = [
        "index",
        "search",
        "find_symbol",
        "find_callers",
        "find_callees",
        "explain",
        "impact",
        "code",
        "remember",
        "recall",
        "compact",
        "retrieve",
        "introspect",
    ]
    .iter()
    .filter_map(|name| find_tool_doc(name))
    .map(tool_doc_detail)
    .collect::<Vec<_>>();

    json!({
        "bootstrap": "# Contextro\n\nStart with `index({\"path\":\"/repo\"})`, then use `search` to find relevant code and `find_symbol` / `find_callers` / `find_callees` / `impact` to trace definitions and dependencies. Use `code` for AST-level symbol inspection, `remember` / `recall` for durable context, and `compact` + `retrieve` when a response is too large to keep inline.\n",
        "parameter_conventions": [
            "Prefer `symbol_name` for symbol tools; `name` and `symbol` aliases still work for backward compatibility.",
            "Use `path` for file and directory scoped tools such as `index`, `analyze`, `focus`, `docs_bundle`, and `sidecar_export`.",
            "Use `query` for search-style tools (`search`, `recall`, `commit_search`) and `tool` for exact `introspect` lookups.",
            "Use `compact` to create an archive ref, then pass the returned `ref_id` (currently `arc_...`) into `retrieve`.",
        ],
        "core_tools": core_tools,
    })
}

/// Look up Contextro's own tool docs.
pub fn handle_introspect(args: &Value) -> Value {
    let tool_filter = args
        .get("tool")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("");
    if !tool_filter.is_empty() {
        return match find_tool_doc(tool_filter) {
            Some(doc) => tool_doc_detail(doc),
            None => json!({"error": format!("Unknown tool: '{}'", tool_filter)}),
        };
    }

    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("");

    if query.is_empty() {
        let all: Vec<Value> = tool_docs().iter().map(tool_doc_summary).collect();
        return json!({"tools": all, "total": all.len()});
    }

    // Match tools where ANY query word appears in name or description.
    // Rank by number of matching words (more matches = more relevant).
    let words: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(|w| w.to_string())
        .collect();

    let mut scored: Vec<(usize, &ToolDoc)> = tool_docs()
        .iter()
        .filter_map(|doc| {
            let haystack = tool_doc_haystack(doc);
            let hits = words
                .iter()
                .filter(|w| haystack.contains(w.as_str()))
                .count();
            if hits > 0 {
                Some((hits, doc))
            } else {
                None
            }
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(b.1.name)));

    let matching: Vec<Value> = scored.iter().map(|(_, doc)| tool_doc_detail(doc)).collect();

    json!({"query": query, "matching_tools": matching, "total": matching.len()})
}

#[cfg(test)]
mod tests {
    use super::*;
    use contextro_core::graph::{
        RelationshipType, UniversalLocation, UniversalNode, UniversalRelationship,
    };
    use contextro_core::NodeType;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("contextro-artifacts-{unique}-{name}"));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn test_introspect_tool_filter_returns_exact_tool_docs() {
        let result = handle_introspect(&json!({"tool":"search"}));
        let parameters = result["parameters"].as_array().expect("parameters array");

        assert_eq!(result["name"], "search");
        assert_eq!(result["tool"], "search");
        assert_eq!(
            result["description"],
            "Hybrid, vector, or BM25 code search."
        );
        assert!(parameters.iter().any(|parameter| parameter.as_str()
            == Some("query (required): search text or symbol-like identifier")));
    }

    #[test]
    fn test_skill_prompt_includes_parameter_docs_and_archive_ref_note() {
        let result = handle_skill_prompt();
        let conventions = result["parameter_conventions"]
            .as_array()
            .expect("parameter conventions array");
        let core_tools = result["core_tools"].as_array().expect("core tools array");

        assert!(conventions
            .iter()
            .any(|note| note.as_str().unwrap_or("").contains("arc_")));
        assert!(core_tools
            .iter()
            .any(|tool| tool["tool"] == "search" && tool["parameters"].is_array()));
    }

    #[test]
    fn test_introspect_list_includes_name_alias() {
        let result = handle_introspect(&json!({}));
        let tools = result["tools"].as_array().expect("tools array");

        assert!(tools
            .iter()
            .any(|tool| tool["name"] == "search" && tool["tool"] == "search"));
    }

    #[test]
    fn test_docs_bundle_writes_rich_overview_markdown() {
        let root = temp_dir("docs-bundle");
        let codebase = root.join("repo");
        std::fs::create_dir_all(codebase.join("src/browser")).unwrap();

        let graph = CodeGraph::new();
        graph.add_node(UniversalNode {
            id: "browser-session".into(),
            name: "BrowserSession".into(),
            node_type: NodeType::Class,
            location: UniversalLocation {
                file_path: codebase
                    .join("src/browser/session.py")
                    .to_string_lossy()
                    .to_string(),
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
            id: "get-session".into(),
            name: "get_or_create_session".into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: codebase
                    .join("src/browser/session.py")
                    .to_string_lossy()
                    .to_string(),
                start_line: 22,
                end_line: 35,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            ..Default::default()
        });
        graph.add_relationship(UniversalRelationship {
            id: "rel-1".into(),
            source_id: "get-session".into(),
            target_id: "browser-session".into(),
            relationship_type: RelationshipType::Calls,
            strength: 1.0,
        });

        let base = codebase.to_string_lossy().to_string();
        let result = handle_docs_bundle(&json!({"output_dir":"docs"}), &graph, Some(base.as_str()));
        assert_eq!(result["status"], "generated");

        let overview = std::fs::read_to_string(codebase.join("docs/overview.md")).unwrap();
        assert!(overview.contains("## Summary"));
        assert!(overview.contains("## Languages"));
        assert!(overview.contains("## Top Files"));
        assert!(overview.contains("## Hub Symbols"));
        assert!(overview.contains("BrowserSession"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_docs_bundle_requires_indexed_graph() {
        let root = temp_dir("docs-bundle-empty");
        let codebase = root.join("repo");
        std::fs::create_dir_all(&codebase).unwrap();
        let graph = CodeGraph::new();

        let base = codebase.to_string_lossy().to_string();
        let result = handle_docs_bundle(&json!({"output_dir":"docs"}), &graph, Some(base.as_str()));

        assert_eq!(
            result["error"],
            "No indexed graph loaded. Run index(path) before docs_bundle."
        );
        assert!(
            !codebase.join("docs/overview.md").exists(),
            "docs bundle should not write placeholder docs when no graph is loaded"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_sidecar_export_matches_relative_indexed_paths() {
        let root = temp_dir("sidecars");
        let codebase = root.join("repo");
        std::fs::create_dir_all(codebase.join("src")).unwrap();
        let output_dir = root.join("out");

        let graph = CodeGraph::new();
        graph.add_node(UniversalNode {
            id: "browser-session".into(),
            name: "BrowserSession".into(),
            node_type: NodeType::Class,
            location: UniversalLocation {
                file_path: "src/session.py".into(),
                start_line: 1,
                end_line: 20,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            ..Default::default()
        });

        let base = codebase.to_string_lossy().to_string();
        let result = handle_sidecar_export(
            &json!({"path":"src","output_dir": output_dir.to_string_lossy()}),
            &graph,
            Some(base.as_str()),
        );

        assert_eq!(result["sidecars"], 1);
        assert!(output_dir.join("src/session.py.graph.md").exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_sidecar_export_errors_when_path_matches_no_indexed_files() {
        let root = temp_dir("sidecars-missing");
        let codebase = root.join("repo");
        std::fs::create_dir_all(codebase.join("src")).unwrap();
        std::fs::create_dir_all(codebase.join("out")).unwrap();

        let graph = CodeGraph::new();
        graph.add_node(UniversalNode {
            id: "browser-session".into(),
            name: "BrowserSession".into(),
            node_type: NodeType::Class,
            location: UniversalLocation {
                file_path: codebase
                    .join("src/session.py")
                    .to_string_lossy()
                    .to_string(),
                start_line: 1,
                end_line: 20,
                start_column: 0,
                end_column: 0,
                language: "python".into(),
            },
            language: "python".into(),
            ..Default::default()
        });

        let base = codebase.to_string_lossy().to_string();
        let result = handle_sidecar_export(
            &json!({"path":"out","output_dir": codebase.join("export").to_string_lossy()}),
            &graph,
            Some(base.as_str()),
        );

        assert!(result["error"]
            .as_str()
            .unwrap_or("")
            .contains("No indexed files matched path"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_skill_prompt_mentions_updated_parameter_contracts() {
        let result = handle_introspect(&json!({"tool":"commit_history"}));
        let parameters = result["parameters"].as_array().expect("parameters array");

        assert!(parameters.iter().any(
            |parameter| parameter.as_str() == Some("author: optional author substring filter")
        ));

        let recall = handle_introspect(&json!({"tool":"recall"}));
        assert!(recall["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .any(|parameter| parameter
                .as_str()
                .unwrap_or("")
                .contains("empty string lists recent memories")));
    }

    #[test]
    fn test_audit_reports_capped_actionable_complexity_evidence() {
        let graph = CodeGraph::new();

        for idx in 0..4 {
            let hub_id = format!("hub-{idx}");
            let hub_name = format!("HubSymbol{idx}");
            graph.add_node(UniversalNode {
                id: hub_id.clone(),
                name: hub_name,
                node_type: NodeType::Function,
                location: UniversalLocation {
                    file_path: format!("src/hub_{idx}.rs"),
                    start_line: 1,
                    end_line: 20,
                    start_column: 0,
                    end_column: 0,
                    language: "rust".into(),
                },
                language: "rust".into(),
                ..Default::default()
            });

            for leaf in 0..11 {
                let leaf_id = format!("leaf-{idx}-{leaf}");
                graph.add_node(UniversalNode {
                    id: leaf_id.clone(),
                    name: format!("Leaf{idx}_{leaf}"),
                    node_type: NodeType::Function,
                    location: UniversalLocation {
                        file_path: format!("src/leaf_{idx}_{leaf}.rs"),
                        start_line: 1,
                        end_line: 5,
                        start_column: 0,
                        end_column: 0,
                        language: "rust".into(),
                    },
                    language: "rust".into(),
                    ..Default::default()
                });
                graph.add_relationship(UniversalRelationship {
                    id: format!("rel-{idx}-{leaf}"),
                    source_id: hub_id.clone(),
                    target_id: leaf_id,
                    relationship_type: RelationshipType::Calls,
                    strength: 1.0,
                });
            }
        }

        graph.add_node(UniversalNode {
            id: "test-hub".into(),
            name: "IgnoredTestHub".into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: "tests/audit_noise.rs".into(),
                start_line: 1,
                end_line: 10,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        });
        for idx in 0..11 {
            let leaf_id = format!("test-leaf-{idx}");
            graph.add_node(UniversalNode {
                id: leaf_id.clone(),
                name: format!("IgnoredLeaf{idx}"),
                node_type: NodeType::Function,
                location: UniversalLocation {
                    file_path: format!("tests/leaf_{idx}.rs"),
                    start_line: 1,
                    end_line: 5,
                    start_column: 0,
                    end_column: 0,
                    language: "rust".into(),
                },
                language: "rust".into(),
                ..Default::default()
            });
            graph.add_relationship(UniversalRelationship {
                id: format!("test-rel-{idx}"),
                source_id: "test-hub".into(),
                target_id: leaf_id,
                relationship_type: RelationshipType::Calls,
                strength: 1.0,
            });
        }

        let result = handle_audit(&graph, None);
        assert_eq!(result["quality_score"], 80);
        let complexity = result["recommendations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["category"] == "complexity")
            .cloned()
            .expect("complexity recommendation");

        assert_eq!(complexity["threshold"], AUDIT_CONNECTION_THRESHOLD);
        assert_eq!(complexity["affected_count"], 4);
        assert!(complexity["message"]
            .as_str()
            .unwrap_or("")
            .contains("top offenders"));

        let evidence = complexity["evidence"].as_array().expect("evidence array");
        assert_eq!(evidence.len(), AUDIT_EVIDENCE_LIMIT);
        assert!(evidence.iter().all(|item| item["connections"] == 11));
        assert!(evidence.iter().all(|item| {
            item["follow_up"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .any(|step| step.as_str().unwrap_or("").contains("explain"))
        }));
        assert!(evidence
            .iter()
            .all(|item| { !item["file"].as_str().unwrap_or("").starts_with("tests/") }));
    }

    #[test]
    fn test_audit_reports_large_file_evidence_without_test_noise() {
        let graph = CodeGraph::new();

        for idx in 0..31 {
            graph.add_node(UniversalNode {
                id: format!("prod-{idx}"),
                name: format!("ProdSymbol{idx}"),
                node_type: NodeType::Function,
                location: UniversalLocation {
                    file_path: "src/big.rs".into(),
                    start_line: idx + 1,
                    end_line: idx + 1,
                    start_column: 0,
                    end_column: 0,
                    language: "rust".into(),
                },
                language: "rust".into(),
                ..Default::default()
            });
        }

        for idx in 0..40 {
            graph.add_node(UniversalNode {
                id: format!("test-{idx}"),
                name: format!("TestSymbol{idx}"),
                node_type: NodeType::Function,
                location: UniversalLocation {
                    file_path: "tests/big_test.rs".into(),
                    start_line: idx + 1,
                    end_line: idx + 1,
                    start_column: 0,
                    end_column: 0,
                    language: "rust".into(),
                },
                language: "rust".into(),
                ..Default::default()
            });
        }

        let result = handle_audit(&graph, None);
        assert_eq!(result["quality_score"], 80);
        let structure = result["recommendations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["category"] == "structure")
            .cloned()
            .expect("structure recommendation");

        assert_eq!(structure["threshold"], AUDIT_FILE_SYMBOL_THRESHOLD);
        assert_eq!(structure["affected_count"], 1);

        let evidence = structure["evidence"].as_array().expect("evidence array");
        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0]["file"], "src/big.rs");
        assert_eq!(evidence[0]["symbols"], 31);
        assert!(evidence[0]["follow_up"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step.as_str().unwrap_or("").contains("analyze")));
    }

    #[test]
    fn test_audit_quality_score_drops_with_worse_hotspots_even_when_categories_are_unchanged() {
        let baseline_graph = CodeGraph::new();
        baseline_graph.add_node(UniversalNode {
            id: "hub".into(),
            name: "Hub".into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: "src/core.rs".into(),
                start_line: 1,
                end_line: 10,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        });
        for idx in 0..11 {
            let leaf_id = format!("baseline-leaf-{idx}");
            baseline_graph.add_node(UniversalNode {
                id: leaf_id.clone(),
                name: format!("BaselineLeaf{idx}"),
                node_type: NodeType::Function,
                location: UniversalLocation {
                    file_path: format!("src/leaf_{idx}.rs"),
                    start_line: 1,
                    end_line: 5,
                    start_column: 0,
                    end_column: 0,
                    language: "rust".into(),
                },
                language: "rust".into(),
                ..Default::default()
            });
            baseline_graph.add_relationship(UniversalRelationship {
                id: format!("baseline-rel-{idx}"),
                source_id: "hub".into(),
                target_id: leaf_id,
                relationship_type: RelationshipType::Calls,
                strength: 1.0,
            });
        }
        for idx in 0..31 {
            baseline_graph.add_node(UniversalNode {
                id: format!("baseline-big-{idx}"),
                name: format!("BaselineBig{idx}"),
                node_type: NodeType::Function,
                location: UniversalLocation {
                    file_path: "src/big.rs".into(),
                    start_line: idx + 1,
                    end_line: idx + 1,
                    start_column: 0,
                    end_column: 0,
                    language: "rust".into(),
                },
                language: "rust".into(),
                ..Default::default()
            });
        }

        let baseline = handle_audit(&baseline_graph, None);
        assert_eq!(baseline["quality_score"], 75);

        let worse_graph = CodeGraph::new();
        worse_graph.add_node(UniversalNode {
            id: "hub".into(),
            name: "Hub".into(),
            node_type: NodeType::Function,
            location: UniversalLocation {
                file_path: "src/core.rs".into(),
                start_line: 1,
                end_line: 10,
                start_column: 0,
                end_column: 0,
                language: "rust".into(),
            },
            language: "rust".into(),
            ..Default::default()
        });
        for idx in 0..31 {
            let leaf_id = format!("worse-leaf-{idx}");
            worse_graph.add_node(UniversalNode {
                id: leaf_id.clone(),
                name: format!("WorseLeaf{idx}"),
                node_type: NodeType::Function,
                location: UniversalLocation {
                    file_path: format!("src/worse_leaf_{idx}.rs"),
                    start_line: 1,
                    end_line: 5,
                    start_column: 0,
                    end_column: 0,
                    language: "rust".into(),
                },
                language: "rust".into(),
                ..Default::default()
            });
            worse_graph.add_relationship(UniversalRelationship {
                id: format!("worse-rel-{idx}"),
                source_id: "hub".into(),
                target_id: leaf_id,
                relationship_type: RelationshipType::Calls,
                strength: 1.0,
            });
        }
        for idx in 0..61 {
            worse_graph.add_node(UniversalNode {
                id: format!("worse-big-{idx}"),
                name: format!("WorseBig{idx}"),
                node_type: NodeType::Function,
                location: UniversalLocation {
                    file_path: "src/big.rs".into(),
                    start_line: idx + 1,
                    end_line: idx + 1,
                    start_column: 0,
                    end_column: 0,
                    language: "rust".into(),
                },
                language: "rust".into(),
                ..Default::default()
            });
        }

        let worse = handle_audit(&worse_graph, None);
        assert_eq!(worse["quality_score"], 72);
        let recommendations = worse["recommendations"].as_array().unwrap();
        assert!(recommendations
            .iter()
            .any(|item| item["category"] == "complexity"));
        assert!(recommendations
            .iter()
            .any(|item| item["category"] == "structure"));
        assert!(recommendations
            .iter()
            .all(|item| item["evidence"].is_array()));
    }
}
