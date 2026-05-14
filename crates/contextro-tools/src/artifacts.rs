//! Artifact tools: audit, docs_bundle, sidecar_export, skill_prompt, introspect.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::analysis::{is_generic_symbol_name, is_test_file, strip_base};
use contextro_engines::graph::CodeGraph;
use serde_json::{json, Value};

#[derive(Clone, Copy)]
struct ToolDoc {
    name: &'static str,
    description: &'static str,
    parameters: &'static [&'static str],
    example: &'static str,
}

const TOOL_DOCS: &[ToolDoc] = &[
    ToolDoc {
        name: "index",
        description: "Index a codebase and build the graph, BM25 index, and vector index.",
        parameters: &["path (required): repository root or codebase directory"],
        example: r#"index({"path":"/repo"})"#,
    },
    ToolDoc {
        name: "search",
        description: "Hybrid, vector, or BM25 code search.",
        parameters: &[
            "query (required): search text or symbol-like identifier",
            "limit: maximum results, default 10",
            "mode: hybrid | vector | bm25",
            "language: optional language filter",
            "context_files: optional file list to boost nearby matches",
        ],
        example: r#"search({"query":"BrowserSession","mode":"bm25"})"#,
    },
    ToolDoc {
        name: "find_symbol",
        description: "Find where a symbol is defined.",
        parameters: &[
            "symbol_name (preferred): exact or fuzzy symbol name",
            "name / symbol: backward-compatible aliases",
            "exact: true for exact match, false for fuzzy lookup",
        ],
        example: r#"find_symbol({"symbol_name":"BrowserSession","exact":true})"#,
    },
    ToolDoc {
        name: "find_callers",
        description: "List the callers of a symbol.",
        parameters: &[
            "symbol_name (preferred): target symbol name",
            "name / symbol: backward-compatible aliases",
        ],
        example: r#"find_callers({"symbol_name":"BrowserSession"})"#,
    },
    ToolDoc {
        name: "find_callees",
        description: "List the callees of a symbol.",
        parameters: &[
            "symbol_name (preferred): target symbol name",
            "name / symbol: backward-compatible aliases",
        ],
        example: r#"find_callees({"symbol_name":"BrowserSession"})"#,
    },
    ToolDoc {
        name: "explain",
        description: "Summarize a symbol with callers, callees, and docstring context.",
        parameters: &[
            "symbol_name (preferred): target symbol name",
            "name / symbol: backward-compatible aliases",
        ],
        example: r#"explain({"symbol_name":"BrowserSession"})"#,
    },
    ToolDoc {
        name: "impact",
        description: "Show the transitive blast radius of changing a symbol.",
        parameters: &[
            "symbol_name (preferred): target symbol name",
            "name / symbol: backward-compatible aliases",
            "max_depth: caller traversal depth, default 5; smaller values intentionally narrow the blast radius",
        ],
        example: r#"impact({"symbol_name":"BrowserSession","max_depth":3})"#,
    },
    ToolDoc {
        name: "overview",
        description: "Show project totals, languages, symbol types, and hotspots.",
        parameters: &[],
        example: r#"overview({})"#,
    },
    ToolDoc {
        name: "architecture",
        description: "Show hub symbols and high-level architectural structure.",
        parameters: &[],
        example: r#"architecture({})"#,
    },
    ToolDoc {
        name: "analyze",
        description: "Show complexity hotspots for a file or directory.",
        parameters: &["path: optional file or directory path to scope the analysis"],
        example: r#"analyze({"path":"crates/contextro-tools/src"})"#,
    },
    ToolDoc {
        name: "focus",
        description: "Return a low-token context slice for a file or directory.",
        parameters: &["path (required): file or directory to summarize"],
        example: r#"focus({"path":"crates/contextro-tools/src/search.rs"})"#,
    },
    ToolDoc {
        name: "dead_code",
        description: "List symbols that appear unreachable from parsed entry points, with optional filters to reduce noise.",
        parameters: &[
            "path: optional file or directory filter",
            "exclude_paths: optional file or directory paths to skip",
            "limit: maximum results, default 50",
            "include_public_api: include likely public API methods/functions (default false)",
            "include_tests: include test files (default false)",
        ],
        example: r#"dead_code({"path":"src","exclude_paths":["src/generated"],"limit":20})"#,
    },
    ToolDoc {
        name: "circular_dependencies",
        description: "Find circular file dependency groups.",
        parameters: &[],
        example: r#"circular_dependencies({})"#,
    },
    ToolDoc {
        name: "test_coverage_map",
        description: "Estimate static test coverage bounds from file naming and inline test markers.",
        parameters: &[],
        example: r#"test_coverage_map({})"#,
    },
    ToolDoc {
        name: "code",
        description: "Run AST-level file and symbol operations.",
        parameters: &[
            "operation (required): get_document_symbols | search_symbols | lookup_symbols | list_symbols | pattern_search | pattern_rewrite | edit_plan | search_codebase_map",
            "path: preferred file or directory path",
            "symbol_name / name / query: operation-specific symbol or filter input",
            "symbols: array of symbol names for lookup_symbols",
            "pattern / replacement / dry_run: rewrite and search parameters",
            "goal: refactoring objective for edit_plan",
            "include_source: include source bodies for lookup_symbols",
        ],
        example: r#"code({"operation":"get_document_symbols","path":"crates/contextro-tools/src/search.rs"})"#,
    },
    ToolDoc {
        name: "remember",
        description: "Store a persistent memory.",
        parameters: &[
            "content (required): text to store",
            "memory_type: note | decision | preference | conversation | status | doc",
            "tags: tag list or comma-separated string",
            "ttl: permanent | session | day | week | month",
        ],
        example: r#"remember({"content":"Use CTX_STORAGE_DIR for RC runs","memory_type":"decision","tags":["release"]})"#,
    },
    ToolDoc {
        name: "recall",
        description: "Search stored memories.",
        parameters: &[
            "query (required): memory search text",
            "limit: maximum results, default 5",
            "memory_type: optional type filter",
            "tags: optional tag filter",
        ],
        example: r#"recall({"query":"release workflow","limit":3})"#,
    },
    ToolDoc {
        name: "tags",
        description: "List all unique memory tags.",
        parameters: &[],
        example: r#"tags({})"#,
    },
    ToolDoc {
        name: "forget",
        description: "Delete stored memories by id, tag, or type.",
        parameters: &[
            "id or memory_id: delete a specific memory",
            "tags: delete memories with matching tags",
            "memory_type: delete memories of a given type",
        ],
        example: r#"forget({"id":"mem_123"})"#,
    },
    ToolDoc {
        name: "knowledge",
        description: "Index lightweight documentation or notes, then search or inspect sources within the active indexed repo scope.",
        parameters: &[
            "command: add | search | show | list | remove | update",
            "name: knowledge base name for add/remove/update",
            "value: raw text or file/directory path for add",
            "query: search text; also auto-triggers search when command is omitted",
            "path: source path for update",
            "limit: maximum results for search, default 5",
            "scope note: results come from the currently active indexed repo",
            "show: detailed source summaries with preview and source_path",
            "list: compact source summary with name and chunk count",
        ],
        example: r#"knowledge({"command":"search","query":"cache invalidation"})"#,
    },
    ToolDoc {
        name: "compact",
        description: "Archive session content for later retrieval.",
        parameters: &[
            "content (required): text to archive",
            "metadata: optional JSON metadata stored with the archive",
        ],
        example: r#"compact({"content":"session summary"})"#,
    },
    ToolDoc {
        name: "session_snapshot",
        description: "Show recent tool calls and captured arguments.",
        parameters: &[],
        example: r#"session_snapshot({})"#,
    },
    ToolDoc {
        name: "restore",
        description: "Show the current codebase path and loaded graph summary.",
        parameters: &[],
        example: r#"restore({})"#,
    },
    ToolDoc {
        name: "retrieve",
        description: "Fetch archived content produced by compact.",
        parameters: &["ref_id (required): archive reference such as arc_ab12cd34"],
        example: r#"retrieve({"ref_id":"arc_ab12cd34"})"#,
    },
    ToolDoc {
        name: "commit_search",
        description: "Search commit history by meaning or keywords.",
        parameters: &[
            "query (required): commit search text",
            "limit: maximum results",
            "author: optional author filter",
        ],
        example: r#"commit_search({"query":"release workflow","limit":5})"#,
    },
    ToolDoc {
        name: "commit_history",
        description: "Show recent commits.",
        parameters: &["limit: maximum commits, default 20"],
        example: r#"commit_history({"limit":10})"#,
    },
    ToolDoc {
        name: "repo_add",
        description: "Register and auto-index an additional repository for multi-repo analysis.",
        parameters: &[
            "path (required): repository directory",
            "name: optional stable label for later removal",
            "behavior: auto-indexes the repo and makes it the active repo scope",
        ],
        example: r#"repo_add({"path":"/tmp/browser-use","name":"browser-use-test"})"#,
    },
    ToolDoc {
        name: "repo_remove",
        description: "Unregister a repository by path or name.",
        parameters: &[
            "path: registered repository path",
            "name: registered repository name",
        ],
        example: r#"repo_remove({"name":"browser-use-test"})"#,
    },
    ToolDoc {
        name: "repo_status",
        description: "List registered repositories.",
        parameters: &[],
        example: r#"repo_status({})"#,
    },
    ToolDoc {
        name: "audit",
        description: "Generate a packaged quality audit report.",
        parameters: &[],
        example: r#"audit({})"#,
    },
    ToolDoc {
        name: "docs_bundle",
        description: "Generate Markdown docs in an output directory from the currently indexed graph.",
        parameters: &[
            "output_dir: target directory, default .contextro-docs",
            "requires an indexed graph: run index(path) first",
        ],
        example: r#"docs_bundle({"output_dir":".contextro-docs"})"#,
    },
    ToolDoc {
        name: "sidecar_export",
        description: "Write .graph.md sidecar files for indexed source files.",
        parameters: &[
            "path: optional source subtree to export",
            "output_dir: optional output directory",
        ],
        example: r#"sidecar_export({"path":"crates/contextro-tools/src"})"#,
    },
    ToolDoc {
        name: "skill_prompt",
        description: "Return the Contextro bootstrap block and parameter conventions.",
        parameters: &[],
        example: r#"skill_prompt({})"#,
    },
    ToolDoc {
        name: "introspect",
        description: "Find the right Contextro tool or inspect one tool's parameters.",
        parameters: &[
            "query: task description for fuzzy matching",
            "tool: exact tool name for detailed parameter docs",
        ],
        example: r#"introspect({"tool":"search"})"#,
    },
    ToolDoc {
        name: "status",
        description: "Show indexing state, graph stats, memory count, and uptime.",
        parameters: &[],
        example: r#"status({})"#,
    },
    ToolDoc {
        name: "health",
        description: "Run a health check.",
        parameters: &[],
        example: r#"health({})"#,
    },
    ToolDoc {
        name: "refactor_check",
        description: "Run definition, callers, callees, impact, and risk analysis in one call.",
        parameters: &[
            "symbol_name (required): symbol to inspect before refactoring",
            "max_depth: impact traversal depth, default 3",
        ],
        example: r#"refactor_check({"symbol_name":"BrowserSession","max_depth":3})"#,
    },
];

fn find_tool_doc(name: &str) -> Option<&'static ToolDoc> {
    TOOL_DOCS
        .iter()
        .find(|doc| doc.name.eq_ignore_ascii_case(name.trim()))
}

fn tool_doc_summary(doc: &ToolDoc) -> Value {
    json!({"tool": doc.name, "description": doc.description})
}

fn tool_doc_detail(doc: &ToolDoc) -> Value {
    json!({
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
    if target_rel.is_empty() {
        return true;
    }
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
pub fn handle_audit(graph: &CodeGraph, _codebase: Option<&str>) -> Value {
    let nodes = graph.find_nodes_by_name("", false);
    let mut recommendations: Vec<Value> = Vec::new();

    // Check for high-complexity symbols
    let mut high_conn = 0;
    for node in &nodes {
        let (in_d, out_d) = graph.get_node_degree(&node.id);
        if in_d + out_d > 10 {
            high_conn += 1;
        }
    }
    if high_conn > 0 {
        recommendations.push(json!({
            "severity": "medium",
            "category": "complexity",
            "message": format!("{} symbols have >10 connections — consider refactoring", high_conn),
        }));
    }

    // Check file concentration
    let mut file_counts: HashMap<String, usize> = HashMap::new();
    for node in &nodes {
        *file_counts
            .entry(node.location.file_path.clone())
            .or_default() += 1;
    }
    let large_files: Vec<_> = file_counts.iter().filter(|(_, c)| **c > 30).collect();
    if !large_files.is_empty() {
        recommendations.push(json!({
            "severity": "low",
            "category": "structure",
            "message": format!("{} files have >30 symbols — consider splitting", large_files.len()),
        }));
    }

    let quality_score = if recommendations.is_empty() {
        95
    } else {
        85 - recommendations.len() * 5
    };

    json!({
        "status": "complete",
        "quality_score": quality_score,
        "total_symbols": nodes.len(),
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

    let nodes = graph.find_nodes_by_name("", false);
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
            let (i, o) = graph.get_node_degree(&n.id);
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

    // Resolve output directory
    let out_base = if Path::new(output_dir).is_absolute() {
        output_dir.to_string()
    } else {
        format!("{}/{}", base, output_dir)
    };
    std::fs::create_dir_all(&out_base).ok();

    let nodes = graph.find_nodes_by_name("", false);
    let mut files_written = 0;

    // Group symbols by file
    let mut by_file: HashMap<String, Vec<&_>> = HashMap::new();
    for node in &nodes {
        if sidecar_target_matches(
            &node.location.file_path,
            &target_abs,
            &target_rel,
            target_is_dir,
            codebase,
        ) {
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
            let (in_d, out_d) = graph.get_node_degree(&sym.id);
            content.push_str(&format!(
                "- `{}` ({}) L{} — {} callers, {} callees\n",
                sym.name, sym.node_type, sym.location.start_line, in_d, out_d
            ));
        }
        if std::fs::write(&sidecar_path, &content).is_ok() {
            files_written += 1;
        }
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
        let all: Vec<Value> = TOOL_DOCS.iter().map(tool_doc_summary).collect();
        return json!({"tools": all, "total": all.len()});
    }

    // Match tools where ANY query word appears in name or description.
    // Rank by number of matching words (more matches = more relevant).
    let words: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(|w| w.to_string())
        .collect();

    let mut scored: Vec<(usize, &ToolDoc)> = TOOL_DOCS
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
}
