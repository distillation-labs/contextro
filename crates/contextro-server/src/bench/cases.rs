use std::path::{Path, PathBuf};

use contextro_core::traits::Parser;
use serde_json::{json, Value};

use super::helpers::{path_to_string, temp_output_dir, temp_path};

#[derive(Clone)]
pub(crate) struct ToolCase {
    pub(crate) display_name: &'static str,
    pub(crate) tool_name: &'static str,
    pub(crate) args: Value,
    pub(crate) notes: &'static str,
    pub(crate) allow_error: bool,
}

pub(crate) struct BenchmarkFixture {
    code_dir_rel: String,
    code_file_rel: String,
    symbol_exact: String,
    commit_query: String,
}

impl BenchmarkFixture {
    pub(crate) fn from_codebase(platform_path: &Path) -> Self {
        let files = contextro_indexing::discover_files(
            platform_path,
            &contextro_config::Settings::default(),
        );
        let code_file = files
            .iter()
            .find(|path| {
                matches!(
                    path.extension().and_then(|ext| ext.to_str()),
                    Some("rs" | "py" | "ts" | "tsx" | "js" | "java" | "go")
                )
            })
            .cloned()
            .unwrap_or_else(|| {
                files
                    .first()
                    .cloned()
                    .expect("codebase contains indexable files")
            });

        let parser = contextro_parsing::TreeSitterParser::new();
        let parsed = parser
            .parse_file(code_file.to_string_lossy().as_ref())
            .expect("parse benchmark fixture file");
        let symbol = parsed
            .symbols
            .iter()
            .find(|symbol| !symbol.name.is_empty())
            .expect("fixture file contains symbols");
        let code_dir_rel = code_file
            .parent()
            .and_then(|parent| parent.strip_prefix(platform_path).ok())
            .map(path_to_string)
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| ".".into());
        let code_file_rel = code_file
            .strip_prefix(platform_path)
            .map(path_to_string)
            .unwrap_or_else(|_| code_file.to_string_lossy().to_string());

        Self {
            code_dir_rel,
            code_file_rel,
            symbol_exact: symbol.name.clone(),
            commit_query: symbol.name.to_ascii_lowercase(),
        }
    }
}

pub(crate) fn build_tool_cases(
    fixture: &BenchmarkFixture,
    codebase: &str,
    retrieve_ref: Option<&str>,
) -> Vec<ToolCase> {
    let knowledge_path = Path::new(codebase).join("README.md");
    let knowledge_value = if knowledge_path.exists() {
        knowledge_path.to_string_lossy().to_string()
    } else {
        "Contextro benchmark note".to_string()
    };
    let retrieve_ref = retrieve_ref.unwrap_or("missing-bench-ref");

    vec![
        ToolCase {
            display_name: "status",
            tool_name: "status",
            args: json!({}),
            notes: "server status",
            allow_error: false,
        },
        ToolCase {
            display_name: "health",
            tool_name: "health",
            args: json!({}),
            notes: "health report",
            allow_error: false,
        },
        ToolCase {
            display_name: "search",
            tool_name: "search",
            args: json!({"query": fixture.symbol_exact, "limit": 5, "mode": "hybrid"}),
            notes: "hybrid search",
            allow_error: false,
        },
        ToolCase {
            display_name: "find_symbol",
            tool_name: "find_symbol",
            args: json!({"symbol_name": fixture.symbol_exact, "exact": true}),
            notes: "exact symbol lookup",
            allow_error: false,
        },
        ToolCase {
            display_name: "find_callers",
            tool_name: "find_callers",
            args: json!({"symbol_name": fixture.symbol_exact, "limit": 10}),
            notes: "graph callers",
            allow_error: false,
        },
        ToolCase {
            display_name: "find_callees",
            tool_name: "find_callees",
            args: json!({"symbol_name": fixture.symbol_exact, "limit": 10}),
            notes: "graph callees",
            allow_error: false,
        },
        ToolCase {
            display_name: "explain",
            tool_name: "explain",
            args: json!({"symbol_name": fixture.symbol_exact}),
            notes: "symbol explanation",
            allow_error: false,
        },
        ToolCase {
            display_name: "impact",
            tool_name: "impact",
            args: json!({"symbol_name": fixture.symbol_exact, "max_depth": 3}),
            notes: "impact analysis",
            allow_error: false,
        },
        ToolCase {
            display_name: "overview",
            tool_name: "overview",
            args: json!({}),
            notes: "project overview",
            allow_error: false,
        },
        ToolCase {
            display_name: "architecture",
            tool_name: "architecture",
            args: json!({"limit": 5}),
            notes: "architecture hubs",
            allow_error: false,
        },
        ToolCase {
            display_name: "analyze",
            tool_name: "analyze",
            args: json!({"path": fixture.code_dir_rel, "top_n": 5}),
            notes: "hotspot analysis",
            allow_error: false,
        },
        ToolCase {
            display_name: "focus",
            tool_name: "focus",
            args: json!({"path": fixture.code_dir_rel}),
            notes: "file focus",
            allow_error: false,
        },
        ToolCase {
            display_name: "dead_code",
            tool_name: "dead_code",
            args: json!({"path": fixture.code_dir_rel, "limit": 5}),
            notes: "dead code heuristic",
            allow_error: false,
        },
        ToolCase {
            display_name: "circular_deps",
            tool_name: "circular_dependencies",
            args: json!({}),
            notes: "cycle detection",
            allow_error: false,
        },
        ToolCase {
            display_name: "test_coverage",
            tool_name: "test_coverage_map",
            args: json!({}),
            notes: "coverage heuristic",
            allow_error: false,
        },
        ToolCase {
            display_name: "code",
            tool_name: "code",
            args: json!({"operation": "get_document_symbols", "path": fixture.code_file_rel}),
            notes: "document symbols",
            allow_error: false,
        },
        ToolCase {
            display_name: "remember",
            tool_name: "remember",
            args: json!({"content": "Contextro benchmark memory", "memory_type": "note", "tags": ["bench"]}),
            notes: "memory write",
            allow_error: false,
        },
        ToolCase {
            display_name: "recall",
            tool_name: "recall",
            args: json!({"query": "benchmark", "limit": 5}),
            notes: "memory recall",
            allow_error: false,
        },
        ToolCase {
            display_name: "tags",
            tool_name: "tags",
            args: json!({}),
            notes: "memory tags",
            allow_error: false,
        },
        ToolCase {
            display_name: "forget",
            tool_name: "forget",
            args: json!({"tags": "bench"}),
            notes: "memory delete by tag",
            allow_error: false,
        },
        ToolCase {
            display_name: "knowledge",
            tool_name: "knowledge",
            args: json!({"command": "add", "name": "bench-doc", "value": knowledge_value}),
            notes: "knowledge add",
            allow_error: false,
        },
        ToolCase {
            display_name: "compact",
            tool_name: "compact",
            args: json!({"content": "Contextro benchmark compact payload"}),
            notes: "session archive",
            allow_error: false,
        },
        ToolCase {
            display_name: "session_snap",
            tool_name: "session_snapshot",
            args: json!({"limit": 5}),
            notes: "session events",
            allow_error: false,
        },
        ToolCase {
            display_name: "restore",
            tool_name: "restore",
            args: json!({}),
            notes: "restore status",
            allow_error: false,
        },
        ToolCase {
            display_name: "retrieve",
            tool_name: "retrieve",
            args: json!({"ref_id": retrieve_ref}),
            notes: "archive lookup",
            allow_error: false,
        },
        ToolCase {
            display_name: "commit_search",
            tool_name: "commit_search",
            args: json!({"query": fixture.commit_query, "limit": 5}),
            notes: "git commit search",
            allow_error: false,
        },
        ToolCase {
            display_name: "commit_history",
            tool_name: "commit_history",
            args: json!({"limit": 5}),
            notes: "git history",
            allow_error: false,
        },
        ToolCase {
            display_name: "repo_status",
            tool_name: "repo_status",
            args: json!({}),
            notes: "repo registry list",
            allow_error: false,
        },
        ToolCase {
            display_name: "audit",
            tool_name: "audit",
            args: json!({}),
            notes: "quality audit",
            allow_error: false,
        },
        ToolCase {
            display_name: "docs_bundle",
            tool_name: "docs_bundle",
            args: json!({"output_dir": temp_output_dir("bench-docs")}),
            notes: "docs bundle",
            allow_error: false,
        },
        ToolCase {
            display_name: "sidecar_export",
            tool_name: "sidecar_export",
            args: json!({"path": fixture.code_dir_rel, "output_dir": temp_output_dir("bench-sidecars")}),
            notes: "sidecar export",
            allow_error: false,
        },
        ToolCase {
            display_name: "skill_prompt",
            tool_name: "skill_prompt",
            args: json!({}),
            notes: "skill prompt",
            allow_error: false,
        },
        ToolCase {
            display_name: "introspect",
            tool_name: "introspect",
            args: json!({"tool": "search"}),
            notes: "tool introspection",
            allow_error: false,
        },
        ToolCase {
            display_name: "refactor_chk",
            tool_name: "refactor_check",
            args: json!({"symbol_name": fixture.symbol_exact, "max_depth": 3}),
            notes: "pre-refactor analysis",
            allow_error: false,
        },
        ToolCase {
            display_name: "completion_chk",
            tool_name: "completion_check",
            args: json!({
                "claim": "all_callers_updated",
                "symbol_name": fixture.symbol_exact,
                "changed_files": [
                    fixture.code_file_rel,
                    format!("{}/nonexistent.rs", fixture.code_dir_rel),
                ],
            }),
            notes: "refactor completeness",
            allow_error: false,
        },
    ]
}

pub(crate) fn temp_fixture_repo() -> PathBuf {
    let root = temp_path("bench-repo-remove");
    std::fs::create_dir_all(root.join("src")).expect("create fixture repo src");
    std::fs::write(root.join("src/lib.rs"), "pub fn bench_fixture() {}\n")
        .expect("write fixture repo file");
    root
}
