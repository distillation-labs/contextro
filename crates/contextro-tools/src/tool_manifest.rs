//! Shared tool manifest for MCP schemas, docs, and tiering.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToolTier {
    Core,
    Standard,
    Full,
}

impl std::str::FromStr for ToolTier {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value.trim().to_ascii_lowercase().as_str() {
            "core" => Self::Core,
            "standard" => Self::Standard,
            _ => Self::Full,
        })
    }
}

impl ToolTier {
    pub fn configured() -> Self {
        std::env::var("CTX_TOOL_TIER")
            .ok()
            .and_then(|raw| raw.parse().ok())
            .unwrap_or(Self::Full)
    }

    pub fn allows(self, doc: &ToolDoc) -> bool {
        doc.min_tier <= self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ToolDoc {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: &'static [&'static str],
    pub example: &'static str,
    pub schema_json: &'static str,
    pub min_tier: ToolTier,
}

const EMPTY_SCHEMA: &str = r#"{"type":"object","properties":{}}"#;
const PATH_SCHEMA: &str = r#"{"type":"object","properties":{"path":{"type":"string","description":"Absolute or relative file or directory path"}}}"#;
const REQUIRED_PATH_SCHEMA: &str = r#"{"type":"object","properties":{"path":{"type":"string","description":"Absolute or relative file or directory path"}},"required":["path"]}"#;
const NAME_SCHEMA: &str = r#"{"type":"object","properties":{"symbol_name":{"type":"string","description":"Preferred symbol name parameter"},"name":{"type":"string","description":"Legacy alias for symbol_name"},"symbol":{"type":"string","description":"Legacy alias for symbol_name"},"exact":{"type":"boolean","description":"true=exact match, false=fuzzy (default: true)"}}}"#;
const SYMBOL_RELATION_SCHEMA: &str = r#"{"type":"object","properties":{"symbol_name":{"type":"string","description":"Preferred symbol name parameter"},"name":{"type":"string","description":"Legacy alias for symbol_name"},"symbol":{"type":"string","description":"Legacy alias for symbol_name"},"limit":{"type":"integer","description":"Maximum results to return (default: 50)"}}}"#;
const QUERY_SCHEMA: &str = r#"{"type":"object","properties":{"query":{"type":"string","description":"Natural language or keyword query"},"limit":{"type":"integer","description":"Max results (default: 10)"},"mode":{"type":"string","description":"bm25 | vector | hybrid (default: hybrid)"},"language":{"type":"string","description":"Filter by language: rust, python, typescript, …"},"context_files":{"oneOf":[{"type":"string"},{"type":"array","items":{"type":"string"}}],"description":"Optional file list to boost nearby matches"}},"required":["query"]}"#;
const IMPACT_SCHEMA: &str = r#"{"type":"object","properties":{"symbol_name":{"type":"string","description":"Preferred symbol name parameter"},"name":{"type":"string","description":"Legacy alias for symbol_name"},"symbol":{"type":"string","description":"Legacy alias for symbol_name"},"max_depth":{"type":"integer","description":"BFS depth (default: 5; smaller values intentionally narrow the blast radius)"}}}"#;
const DEAD_CODE_SCHEMA: &str = r#"{"type":"object","properties":{"path":{"type":"string","description":"Optional file or directory filter"},"exclude_paths":{"type":"array","items":{"type":"string"},"description":"Optional file or directory paths to exclude"},"limit":{"type":"integer","description":"Max results (default: 50)"},"include_public_api":{"type":"boolean","description":"Include likely public API methods/functions in the output (default: false)"},"include_tests":{"type":"boolean","description":"Include test files in the output (default: false)"}}}"#;
const CODE_SCHEMA: &str = r#"{"type":"object","properties":{"operation":{"type":"string","description":"get_document_symbols | search_symbols | lookup_symbols | list_symbols | pattern_search | pattern_rewrite | edit_plan | search_codebase_map"},"path":{"type":"string","description":"Preferred file or directory path parameter"},"file_path":{"type":"string","description":"Legacy alias for path"},"symbol_name":{"type":"string","description":"Preferred symbol name parameter"},"name":{"type":"string","description":"Legacy alias for symbol_name"},"symbols":{"type":"array","items":{"type":"string"},"description":"Array of symbol names (lookup_symbols); comma-string also accepted"},"pattern":{"type":"string","description":"Regex or ast-grep pattern (pattern_search, pattern_rewrite)"},"query":{"type":"string","description":"Operation-specific query or search alias"},"language":{"type":"string","description":"Language filter for pattern_search / pattern_rewrite"},"replacement":{"type":"string","description":"Replacement string (pattern_rewrite)"},"dry_run":{"type":"boolean","description":"Preview only, no writes (pattern_rewrite, default: true)"},"goal":{"type":"string","description":"Refactoring goal description (edit_plan)"},"include_source":{"type":"boolean","description":"Include source code in lookup_symbols (default: false)"},"include_signature":{"type":"boolean","description":"Include truncated signatures in get_document_symbols or file-path list_symbols output (default: false)"},"limit":{"type":"integer","description":"Optional result cap override for get_document_symbols or search results in code operations"}},"required":["operation"]}"#;
const MEMORY_SCHEMA: &str = r#"{"type":"object","properties":{"content":{"type":"string","description":"Text to store"},"memory_type":{"type":"string","description":"note | decision | preference | conversation | status | doc"},"tags":{"type":"array","items":{"type":"string"},"description":"Tag list; comma-string also accepted"},"ttl":{"type":"string","description":"permanent | session | day | week | month"}},"required":["content"]}"#;
const RECALL_SCHEMA: &str = r#"{"type":"object","properties":{"query":{"type":"string","description":"What to search for in memories. Empty string lists recent memories."},"limit":{"type":"integer","description":"Max results (default: 5)"},"memory_type":{"type":"string","description":"Filter by type: note, decision, …"},"tags":{"oneOf":[{"type":"string"},{"type":"array","items":{"type":"string"}}],"description":"Filter by tag (string or array)"}}}"#;
const KNOWLEDGE_SCHEMA: &str = r#"{"type":"object","properties":{"command":{"type":"string","description":"add | search | show | list | remove | update | clear (omit to auto-detect from query)"},"name":{"type":"string","description":"Knowledge base name (add, remove, update)"},"query":{"type":"string","description":"Search query (search); also triggers search when command is omitted"},"value":{"type":"string","description":"Inline content or an existing file/directory path to index (add)"},"path":{"type":"string","description":"Existing file/directory path to re-index for update"},"limit":{"type":"integer","description":"Max results (search, default: 5)"}}}"#;
const REF_ID_SCHEMA: &str = r#"{"type":"object","properties":{"ref_id":{"type":"string","description":"Reference ID returned by compact"}},"required":["ref_id"]}"#;
const COMMIT_SEARCH_SCHEMA: &str = r#"{"type":"object","properties":{"query":{"type":"string","description":"Keywords or description to search commit messages"},"limit":{"type":"integer","description":"Max results"},"author":{"type":"string","description":"Filter by author name"}},"required":["query"]}"#;
const COMMIT_HISTORY_SCHEMA: &str = r#"{"type":"object","properties":{"limit":{"type":"integer","description":"Number of commits to return (default: 20)"},"since":{"type":"string","description":"Only return commits on or after this timestamp/date (RFC3339 or YYYY-MM-DD)"},"author":{"type":"string","description":"Only return commits whose author matches this string"}}}"#;
const ARCHITECTURE_SCHEMA: &str = r#"{"type":"object","properties":{"limit":{"type":"integer","description":"Maximum hub symbols to return (default: 10)"}}}"#;
const ANALYZE_SCHEMA: &str = r#"{"type":"object","properties":{"path":{"type":"string","description":"Absolute or relative file or directory path"},"min_connections":{"type":"integer","description":"Minimum connectivity threshold for hotspot reporting (default: 6)"},"top_n":{"type":"integer","description":"Maximum hotspot symbols to return (default: 10)"}}}"#;
const FORGET_SCHEMA: &str = r#"{"type":"object","properties":{"id":{"type":"string","description":"ID returned by remember()"},"memory_id":{"type":"string","description":"Legacy alias for the memory ID"},"tags":{"type":"string","description":"Delete all memories with this tag"},"memory_type":{"type":"string","description":"Delete all memories of this type"}}}"#;
const COMPACT_SCHEMA: &str = r#"{"type":"object","properties":{"content":{"type":"string","description":"Session content to archive"},"metadata":{"type":"object","description":"Optional metadata stored with the archive entry"},"ttl":{"type":"string","description":"Requested visibility TTL: permanent | session | day | week | month"}},"required":["content"]}"#;
const SESSION_SNAPSHOT_SCHEMA: &str = r#"{"type":"object","properties":{"limit":{"type":"integer","description":"Maximum events to return (default: 20)"},"type":{"type":"string","description":"Optional event type filter such as search or index"}}}"#;
const REPO_REMOVE_SCHEMA: &str = r#"{"type":"object","properties":{"path":{"type":"string","description":"Registered repository path"},"name":{"type":"string","description":"Registered repository name"}}}"#;
const DOCS_BUNDLE_SCHEMA: &str = r#"{"type":"object","properties":{"output_dir":{"type":"string","description":"Output directory for generated docs (default: .contextro-docs)"}}}"#;
const SIDECAR_EXPORT_SCHEMA: &str = r#"{"type":"object","properties":{"path":{"type":"string","description":"Indexed source file or directory to export"},"output_dir":{"type":"string","description":"Directory to write .graph.* sidecar files (default: .contextro-sidecars)"}}}"#;
const INTROSPECT_SCHEMA: &str = r#"{"type":"object","properties":{"query":{"type":"string","description":"Describe what you want to do"},"tool":{"type":"string","description":"Exact tool name for parameter docs and examples"}}}"#;
const REFACTOR_CHECK_SCHEMA: &str = r#"{"type":"object","properties":{"symbol_name":{"type":"string","description":"Symbol to analyze before refactoring"},"max_depth":{"type":"integer","description":"BFS depth for impact (default: 3)"}},"required":["symbol_name"]}"#;
const COMPLETION_CHECK_SCHEMA: &str = r#"{"type":"object","properties":{"claim":{"type":"string","description":"What kind of completeness to verify. Currently supported: all_callers_updated"},"symbol_name":{"type":"string","description":"The symbol being refactored (renamed, signature-changed, etc.)"},"changed_files":{"type":"array","items":{"type":"string"},"description":"All files touched by the refactor (caller files + definition file)"},"max_depth":{"type":"integer","description":"Transitive caller depth for future claims (reserved, defaults to 0)"}},"required":["claim","symbol_name","changed_files"]}"#;

const TOOL_DOCS: &[ToolDoc] = &[
    ToolDoc {
        name: "status",
        description: "Show indexing state, graph stats, memory count, and uptime.",
        parameters: &[],
        example: r#"status({})"#,
        schema_json: EMPTY_SCHEMA,
        min_tier: ToolTier::Core,
    },
    ToolDoc {
        name: "health",
        description: "Run a health check.",
        parameters: &[],
        example: r#"health({})"#,
        schema_json: EMPTY_SCHEMA,
        min_tier: ToolTier::Core,
    },
    ToolDoc {
        name: "index",
        description: "Index a codebase and build the graph, BM25 index, and vector index.",
        parameters: &["path (required): repository root or codebase directory"],
        example: r#"index({"path":"/repo"})"#,
        schema_json: REQUIRED_PATH_SCHEMA,
        min_tier: ToolTier::Core,
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
        schema_json: QUERY_SCHEMA,
        min_tier: ToolTier::Core,
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
        schema_json: NAME_SCHEMA,
        min_tier: ToolTier::Core,
    },
    ToolDoc {
        name: "find_callers",
        description: "List the callers of a symbol.",
        parameters: &[
            "symbol_name (preferred): target symbol name",
            "name / symbol: backward-compatible aliases",
            "limit: maximum callers to return, default 50",
        ],
        example: r#"find_callers({"symbol_name":"BrowserSession"})"#,
        schema_json: SYMBOL_RELATION_SCHEMA,
        min_tier: ToolTier::Core,
    },
    ToolDoc {
        name: "find_callees",
        description: "List the callees of a symbol.",
        parameters: &[
            "symbol_name (preferred): target symbol name",
            "name / symbol: backward-compatible aliases",
            "limit: maximum callees to return, default 50",
        ],
        example: r#"find_callees({"symbol_name":"BrowserSession"})"#,
        schema_json: SYMBOL_RELATION_SCHEMA,
        min_tier: ToolTier::Core,
    },
    ToolDoc {
        name: "explain",
        description: "Summarize a symbol with callers, callees, and docstring context.",
        parameters: &[
            "symbol_name (preferred): target symbol name",
            "name / symbol: backward-compatible aliases",
        ],
        example: r#"explain({"symbol_name":"BrowserSession"})"#,
        schema_json: SYMBOL_RELATION_SCHEMA,
        min_tier: ToolTier::Core,
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
        schema_json: IMPACT_SCHEMA,
        min_tier: ToolTier::Core,
    },
    ToolDoc {
        name: "overview",
        description: "Show project totals, languages, symbol types, and hotspots.",
        parameters: &[],
        example: r#"overview({})"#,
        schema_json: EMPTY_SCHEMA,
        min_tier: ToolTier::Standard,
    },
    ToolDoc {
        name: "architecture",
        description: "Show hub symbols and high-level architectural structure.",
        parameters: &["limit: maximum hub symbols to return, default 10"],
        example: r#"architecture({})"#,
        schema_json: ARCHITECTURE_SCHEMA,
        min_tier: ToolTier::Standard,
    },
    ToolDoc {
        name: "analyze",
        description: "Show complexity hotspots for a file or directory.",
        parameters: &[
            "path: optional file or directory path to scope the analysis",
            "min_connections: minimum connectivity threshold for hotspot reporting",
            "top_n: maximum high-connectivity symbols to return, default 10",
        ],
        example: r#"analyze({"path":"crates/contextro-tools/src"})"#,
        schema_json: ANALYZE_SCHEMA,
        min_tier: ToolTier::Standard,
    },
    ToolDoc {
        name: "focus",
        description: "Return a low-token context slice for a file or directory.",
        parameters: &["path (required): file or directory to summarize"],
        example: r#"focus({"path":"crates/contextro-tools/src/search.rs"})"#,
        schema_json: PATH_SCHEMA,
        min_tier: ToolTier::Standard,
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
        schema_json: DEAD_CODE_SCHEMA,
        min_tier: ToolTier::Standard,
    },
    ToolDoc {
        name: "circular_dependencies",
        description: "Find circular file dependency groups.",
        parameters: &[],
        example: r#"circular_dependencies({})"#,
        schema_json: EMPTY_SCHEMA,
        min_tier: ToolTier::Full,
    },
    ToolDoc {
        name: "test_coverage_map",
        description: "Estimate static test coverage bounds from file naming and inline test markers.",
        parameters: &[],
        example: r#"test_coverage_map({})"#,
        schema_json: EMPTY_SCHEMA,
        min_tier: ToolTier::Full,
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
            "include_signature: include truncated signatures in get_document_symbols or file-path list_symbols output",
            "limit: optional override when you want more than the compact default from get_document_symbols",
        ],
        example: r#"code({"operation":"get_document_symbols","path":"crates/contextro-tools/src/search.rs","limit":40}) // returns {file, columns, symbols, total}"#,
        schema_json: CODE_SCHEMA,
        min_tier: ToolTier::Core,
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
        schema_json: MEMORY_SCHEMA,
        min_tier: ToolTier::Standard,
    },
    ToolDoc {
        name: "recall",
        description: "Search stored memories.",
        parameters: &[
            "query: memory search text; empty string lists recent memories",
            "limit: maximum results, default 5",
            "memory_type: optional type filter",
            "tags: optional tag filter",
        ],
        example: r#"recall({"query":"release workflow","limit":3})"#,
        schema_json: RECALL_SCHEMA,
        min_tier: ToolTier::Standard,
    },
    ToolDoc {
        name: "tags",
        description: "List all unique memory tags.",
        parameters: &[],
        example: r#"tags({})"#,
        schema_json: EMPTY_SCHEMA,
        min_tier: ToolTier::Full,
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
        schema_json: FORGET_SCHEMA,
        min_tier: ToolTier::Standard,
    },
    ToolDoc {
        name: "knowledge",
        description: "Index lightweight documentation or notes, then search or inspect sources within the active indexed repo scope.",
        parameters: &[
            "command: add | search | show | list | remove | update | clear",
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
        schema_json: KNOWLEDGE_SCHEMA,
        min_tier: ToolTier::Standard,
    },
    ToolDoc {
        name: "compact",
        description: "Archive session content for later retrieval.",
        parameters: &[
            "content (required): text to archive",
            "metadata: optional JSON metadata stored with the archive",
            "ttl: requested retention hint for observability (permanent | session | day | week | month)",
        ],
        example: r#"compact({"content":"session summary"})"#,
        schema_json: COMPACT_SCHEMA,
        min_tier: ToolTier::Full,
    },
    ToolDoc {
        name: "session_snapshot",
        description: "Show recent tool calls and captured arguments.",
        parameters: &[
            "limit: maximum events to return, default 20",
            "type: optional event type filter such as search or index",
        ],
        example: r#"session_snapshot({})"#,
        schema_json: SESSION_SNAPSHOT_SCHEMA,
        min_tier: ToolTier::Full,
    },
    ToolDoc {
        name: "restore",
        description: "Show the current codebase path and loaded graph summary.",
        parameters: &[],
        example: r#"restore({})"#,
        schema_json: EMPTY_SCHEMA,
        min_tier: ToolTier::Full,
    },
    ToolDoc {
        name: "retrieve",
        description: "Fetch archived content produced by compact.",
        parameters: &["ref_id (required): archive reference such as arc_ab12cd34"],
        example: r#"retrieve({"ref_id":"arc_ab12cd34"})"#,
        schema_json: REF_ID_SCHEMA,
        min_tier: ToolTier::Full,
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
        schema_json: COMMIT_SEARCH_SCHEMA,
        min_tier: ToolTier::Standard,
    },
    ToolDoc {
        name: "commit_history",
        description: "Show recent commits.",
        parameters: &[
            "limit: maximum commits, default 20",
            "author: optional author substring filter",
            "since: optional RFC3339 or YYYY-MM-DD lower time bound",
        ],
        example: r#"commit_history({"limit":10})"#,
        schema_json: COMMIT_HISTORY_SCHEMA,
        min_tier: ToolTier::Standard,
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
        schema_json: REQUIRED_PATH_SCHEMA,
        min_tier: ToolTier::Full,
    },
    ToolDoc {
        name: "repo_remove",
        description: "Unregister a repository by path or name.",
        parameters: &[
            "path: registered repository path",
            "name: registered repository name",
        ],
        example: r#"repo_remove({"name":"browser-use-test"})"#,
        schema_json: REPO_REMOVE_SCHEMA,
        min_tier: ToolTier::Full,
    },
    ToolDoc {
        name: "repo_status",
        description: "List registered repositories.",
        parameters: &[],
        example: r#"repo_status({})"#,
        schema_json: EMPTY_SCHEMA,
        min_tier: ToolTier::Full,
    },
    ToolDoc {
        name: "audit",
        description: "Generate a packaged quality audit report.",
        parameters: &[],
        example: r#"audit({})"#,
        schema_json: EMPTY_SCHEMA,
        min_tier: ToolTier::Full,
    },
    ToolDoc {
        name: "docs_bundle",
        description: "Generate Markdown docs in an output directory from the currently indexed graph.",
        parameters: &[
            "output_dir: target directory, default .contextro-docs",
            "requires an indexed graph: run index(path) first",
        ],
        example: r#"docs_bundle({"output_dir":".contextro-docs"})"#,
        schema_json: DOCS_BUNDLE_SCHEMA,
        min_tier: ToolTier::Full,
    },
    ToolDoc {
        name: "sidecar_export",
        description: "Write .graph.md sidecar files for indexed source files.",
        parameters: &[
            "path: optional indexed source file or subtree to export",
            "output_dir: optional output directory",
        ],
        example: r#"sidecar_export({"path":"crates/contextro-tools/src"})"#,
        schema_json: SIDECAR_EXPORT_SCHEMA,
        min_tier: ToolTier::Full,
    },
    ToolDoc {
        name: "skill_prompt",
        description: "Return the Contextro bootstrap block and parameter conventions.",
        parameters: &[],
        example: r#"skill_prompt({})"#,
        schema_json: EMPTY_SCHEMA,
        min_tier: ToolTier::Full,
    },
    ToolDoc {
        name: "introspect",
        description: "Find the right Contextro tool or inspect one tool's parameters.",
        parameters: &[
            "query: task description for fuzzy matching",
            "tool: exact tool name for detailed parameter docs",
        ],
        example: r#"introspect({"tool":"search"})"#,
        schema_json: INTROSPECT_SCHEMA,
        min_tier: ToolTier::Standard,
    },
    ToolDoc {
        name: "refactor_check",
        description: "Run definition, callers, callees, impact, and risk analysis in one call.",
        parameters: &[
            "symbol_name (required): symbol to inspect before refactoring",
            "max_depth: impact traversal depth, default 3",
        ],
        example: r#"refactor_check({"symbol_name":"BrowserSession","max_depth":3})"#,
        schema_json: REFACTOR_CHECK_SCHEMA,
        min_tier: ToolTier::Full,
    },
    ToolDoc {
        name: "completion_check",
        description: "Verify that a refactor is complete by checking the code graph against claimed changed files.",
        parameters: &[
            "claim (required): type of completeness check — all_callers_updated",
            "symbol_name (required): the symbol being refactored",
            "changed_files (required): list of files touched by the refactor",
            "max_depth: reserved for future transitive claims, defaults to 0",
        ],
        example: r#"completion_check({"claim":"all_callers_updated","symbol_name":"BrowserSession","changed_files":["src/session.rs","src/main.rs"]})"#,
        schema_json: COMPLETION_CHECK_SCHEMA,
        min_tier: ToolTier::Full,
    },
];

pub fn tool_docs() -> &'static [ToolDoc] {
    TOOL_DOCS
}

pub fn tool_docs_for_tier(tier: ToolTier) -> Vec<&'static ToolDoc> {
    TOOL_DOCS.iter().filter(|doc| tier.allows(doc)).collect()
}

pub fn find_tool_doc(name: &str) -> Option<&'static ToolDoc> {
    TOOL_DOCS
        .iter()
        .find(|doc| doc.name.eq_ignore_ascii_case(name.trim()))
}

#[cfg(test)]
mod tests {
    use super::{find_tool_doc, tool_docs_for_tier, ToolTier};

    #[test]
    fn standard_tier_excludes_full_only_tools() {
        let names: Vec<&str> = tool_docs_for_tier(ToolTier::Standard)
            .into_iter()
            .map(|doc| doc.name)
            .collect();

        assert!(names.contains(&"search"));
        assert!(names.contains(&"introspect"));
        assert!(!names.contains(&"audit"));
        assert!(!names.contains(&"docs_bundle"));
    }

    #[test]
    fn find_tool_doc_is_case_insensitive() {
        let search = find_tool_doc("SeArCh").expect("search tool manifest entry");
        assert_eq!(search.name, "search");
    }
}
