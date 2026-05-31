use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use contextro_engines::bm25::Bm25Engine;
use parking_lot::RwLock;

use super::response_utils::format_response;
use super::*;

fn process_repo_bm25_cache() -> &'static RwLock<HashMap<String, Arc<Bm25Engine>>> {
    static PROCESS_REPO_BM25: OnceLock<RwLock<HashMap<String, Arc<Bm25Engine>>>> = OnceLock::new();
    PROCESS_REPO_BM25.get_or_init(|| RwLock::new(HashMap::new()))
}

pub(super) fn edit_distance(a: &str, b: &str) -> usize {
    let (m, n) = (a.len(), b.len());
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0; n + 1];
    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(curr[j] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

impl Default for ContextroServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Scan `root` for project documentation files and add them to the knowledge store.
/// Returns the number of documents indexed. Does nothing if the KB already has content.
pub(super) fn auto_populate_knowledge(
    root: &str,
    knowledge: &contextro_tools::KnowledgeStore,
) -> usize {
    let candidates = [
        "README.md",
        "README.txt",
        "README",
        "CLAUDE.md",
        "AGENTS.md",
        "docs/README.md",
        "docs/index.md",
        "CONTRIBUTING.md",
    ];
    if candidates.iter().any(|name| knowledge.contains(name)) {
        return 0; // KB already has seeded docs for this scope; don't overwrite
    }
    let docs = candidates
        .iter()
        .filter_map(|name| {
            let path = std::path::Path::new(root).join(name);
            let content = std::fs::read_to_string(&path).ok()?;
            Some((name.to_string(), content, Some(path)))
        })
        .collect::<Vec<_>>();
    knowledge.add_documents(docs)
}

pub(super) fn should_build_vector_index(chunk_count: usize) -> bool {
    chunk_count > 1
}

pub(super) fn should_share_repo_bm25(chunk_count: usize) -> bool {
    (1..=8).contains(&chunk_count)
}

pub(super) fn process_repo_bm25(path: &str) -> Option<Arc<Bm25Engine>> {
    process_repo_bm25_cache().read().get(path).cloned()
}

pub(super) fn remember_process_repo_bm25(path: String, bm25: Arc<Bm25Engine>) {
    process_repo_bm25_cache().write().insert(path, bm25);
}

pub(super) fn prune_process_repo_bm25(path: &str) {
    process_repo_bm25_cache().write().remove(path);
}

pub(super) fn response_cache_key(
    name: &str,
    args: &Value,
    codebase: Option<&str>,
) -> Option<String> {
    if !is_response_cacheable_tool(name) {
        return None;
    }

    let args = strip_render_only_args(args);
    Some(
        json!({
            "tool": name,
            "args": args,
            "codebase": codebase.unwrap_or(""),
        })
        .to_string(),
    )
}

fn is_response_cacheable_tool(name: &str) -> bool {
    matches!(
        name,
        "overview"
            | "architecture"
            | "analyze"
            | "focus"
            | "dead_code"
            | "circular_dependencies"
            | "test_coverage_map"
            | "audit"
    )
}

pub(super) fn strip_render_only_args(args: &Value) -> Value {
    match args {
        Value::Object(map) => {
            let mut cleaned = map.clone();
            cleaned.remove("max_tokens");
            Value::Object(cleaned)
        }
        other => other.clone(),
    }
}

pub(super) fn cached_tool_result(
    result: &Value,
    rendered_default: Option<&str>,
    args: &Value,
) -> CallToolResult {
    let max_tokens = args.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let rendered = if max_tokens == 0 {
        rendered_default
            .map(str::to_owned)
            .unwrap_or_else(|| format_response(result, 0))
    } else {
        format_response(result, max_tokens)
    };
    CallToolResult::success(vec![Content::text(rendered)])
}

pub(super) fn maybe_prewarm_commit_search_cache(path: &str) {
    if !get_settings().read().search_prewarm_enabled {
        return;
    }

    contextro_tools::git_tools::prewarm_commit_search_cache(Some(path));
}

impl ServerHandler for ContextroServer {
    fn get_info(&self) -> InitializeResult {
        InitializeResult {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "contextro".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            instructions: Some("Contextro: code intelligence MCP server. 37 tools for search, graph analysis, memory, and git.".into()),
        }
    }

    fn list_tools(
        &self,
        _request: PaginatedRequestParam,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        std::future::ready(Ok(ListToolsResult {
            tools: Self::listed_tool_definitions(),
            next_cursor: None,
        }))
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        let args = request.arguments.map(Value::Object).unwrap_or(Value::Null);
        let result = self.dispatch(&request.name, args);
        std::future::ready(Ok(result))
    }
}

pub(super) fn normalize_repo_dir(path: &str) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| std::path::PathBuf::from(path))
        .to_string_lossy()
        .to_string()
}

pub(super) fn resolve_refactor_targets(
    name: &str,
    graph: &contextro_engines::graph::CodeGraph,
) -> Vec<contextro_core::graph::UniversalNode> {
    let exact = graph.find_nodes_by_name(name, true);
    if !exact.is_empty() {
        return rank_nodes_by_degree(exact, graph);
    }
    rank_nodes_by_degree(graph.find_nodes_by_name(name, false), graph)
}

fn rank_nodes_by_degree(
    mut nodes: Vec<contextro_core::graph::UniversalNode>,
    graph: &contextro_engines::graph::CodeGraph,
) -> Vec<contextro_core::graph::UniversalNode> {
    nodes.sort_by_key(|node| {
        let (in_degree, out_degree) = graph.get_node_degree(&node.id);
        std::cmp::Reverse(in_degree + out_degree)
    });
    nodes
}
