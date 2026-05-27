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

mod audit;

pub use audit::handle_audit;
#[cfg(test)]
pub(crate) use audit::{
    AUDIT_CONNECTION_THRESHOLD, AUDIT_EVIDENCE_LIMIT, AUDIT_FILE_SYMBOL_THRESHOLD,
};

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
mod tests;
