//! Contextro MCP server binary — single compiled Rust binary.

use std::sync::Arc;

use anyhow::Result;
use rmcp::model::*;
use rmcp::Error as McpError;
use rmcp::{ServerHandler, ServiceExt};
use serde_json::{json, Value};
use tracing::info;

use contextro_config::get_settings;

mod http;
mod state;
mod update_check;
use state::AppState;

/// The Contextro MCP server.
#[derive(Clone)]
pub struct ContextroServer {
    state: Arc<AppState>,
}

impl ContextroServer {
    pub fn new() -> Self {
        Self {
            state: Arc::new(AppState::new()),
        }
    }

    fn dispatch(&self, name: &str, args: Value) -> CallToolResult {
        let s = &self.state;
        let codebase = s.codebase_path.read().clone();
        let cb = codebase.as_deref();

        let result = match name {
            "status" => self.handle_status(),
            "health" => self.handle_health(),
            "index" => self.handle_index(&args),
            "search" => {
                contextro_tools::search::handle_search(&args, &s.bm25, &s.graph, &s.query_cache, &s.vector_index)
            }
            "find_symbol" => self.handle_find_symbol(&args),
            "find_callers" => {
                contextro_tools::graph_tools::handle_find_callers(&args, &s.graph, cb)
            }
            "find_callees" => {
                contextro_tools::graph_tools::handle_find_callees(&args, &s.graph, cb)
            }
            "explain" => contextro_tools::graph_tools::handle_explain(&args, &s.graph, cb),
            "impact" => contextro_tools::graph_tools::handle_impact(&args, &s.graph, cb),
            "overview" => contextro_tools::analysis::handle_overview(
                &s.graph,
                cb,
                s.chunk_count.load(std::sync::atomic::Ordering::Relaxed),
            ),
            "architecture" => contextro_tools::analysis::handle_architecture(&s.graph, cb),
            "analyze" => contextro_tools::analysis::handle_analyze(&args, &s.graph, cb),
            "focus" => contextro_tools::analysis::handle_focus(&args, &s.graph, cb),
            "dead_code" => contextro_tools::analysis::handle_dead_code(&s.graph, cb),
            "circular_dependencies" => {
                contextro_tools::analysis::handle_circular_dependencies(&s.graph, cb)
            }
            "test_coverage_map" => {
                contextro_tools::analysis::handle_test_coverage_map(&s.graph, cb)
            }
            "remember" => contextro_tools::memory::handle_remember(&args, &s.memory_store),
            "recall" => contextro_tools::memory::handle_recall(&args, &s.memory_store),
            "forget" => contextro_tools::memory::handle_forget(&args, &s.memory_store),
            "knowledge" => contextro_tools::memory::handle_knowledge(&args, &s.knowledge),
            "compact" => contextro_tools::session::handle_compact(&args, &s.archive),
            "session_snapshot" => {
                contextro_tools::session::handle_session_snapshot(&s.session_tracker)
            }
            "restore" => contextro_tools::session::handle_restore(
                cb,
                s.graph.node_count(),
                s.graph.relationship_count(),
            ),
            "retrieve" => contextro_tools::session::handle_retrieve(&args, &s.archive),
            "commit_search" => contextro_tools::git_tools::handle_commit_search(&args, cb),
            "commit_history" => contextro_tools::git_tools::handle_commit_history(&args, cb),
            "repo_add" => contextro_tools::git_tools::handle_repo_add(&args, &s.repo_registry),
            "repo_remove" => {
                contextro_tools::git_tools::handle_repo_remove(&args, &s.repo_registry)
            }
            "repo_status" => contextro_tools::git_tools::handle_repo_status(&s.repo_registry),
            "code" => contextro_tools::code::handle_code(&args, &s.graph, cb),
            "audit" => contextro_tools::artifacts::handle_audit(&s.graph, cb),
            "docs_bundle" => contextro_tools::artifacts::handle_docs_bundle(&args, &s.graph, cb),
            "sidecar_export" => {
                contextro_tools::artifacts::handle_sidecar_export(&args, &s.graph, cb)
            }
            "skill_prompt" => contextro_tools::artifacts::handle_skill_prompt(),
            "introspect" => contextro_tools::artifacts::handle_introspect(&args),
            _ => json!({"error": format!("Unknown tool: {}", name)}),
        };

        s.session_tracker.track(name, &format!("{}()", name));

        if result.get("error").is_some() {
            CallToolResult::error(vec![Content::text(result.to_string())])
        } else {
            CallToolResult::success(vec![Content::text(result.to_string())])
        }
    }

    fn handle_status(&self) -> Value {
        let uptime = self.state.started_at.elapsed().as_secs_f64();
        json!({
            "indexed": *self.state.indexed.read(),
            "codebase_path": *self.state.codebase_path.read(),
            "uptime_seconds": (uptime * 10.0).round() / 10.0,
            "graph_nodes": self.state.graph.node_count(),
            "graph_relationships": self.state.graph.relationship_count(),
            "cache_hit_rate": self.state.query_cache.hit_rate(),
            "memories": self.state.memory_store.count(),
        })
    }

    fn handle_health(&self) -> Value {
        json!({
            "status": "healthy",
            "uptime_seconds": (self.state.started_at.elapsed().as_secs_f64() * 10.0).round() / 10.0,
            "indexed": *self.state.indexed.read(),
        })
    }

    fn handle_index(&self, args: &Value) -> Value {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        if path.is_empty() {
            return json!({"error": "Missing required parameter: path"});
        }
        if !std::path::Path::new(path).is_dir() {
            return json!({"error": format!("Not a directory: {}", path)});
        }

        let settings = get_settings().read().clone();
        let pipeline = contextro_indexing::IndexingPipeline::new(settings);

        match pipeline.index(std::path::Path::new(path)) {
            Ok((result, symbols)) => {
                self.state.graph.clear();
                self.state.build_graph(&symbols);
                self.state.bm25.clear();

                let chunks = contextro_indexing::create_chunks(&symbols);
                self.state.bm25.index_chunks(&chunks);
                self.state
                    .chunk_count
                    .store(chunks.len(), std::sync::atomic::Ordering::Relaxed);

                // Populate vector index
                self.state.vector_index.clear();
                let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
                if let Some(vectors) = contextro_indexing::embed_batch(&texts) {
                    for (chunk, vector) in chunks.iter().zip(vectors.into_iter()) {
                        let result = contextro_core::models::SearchResult {
                            id: chunk.id.clone(),
                            filepath: chunk.filepath.clone(),
                            symbol_name: chunk.symbol_name.clone(),
                            symbol_type: chunk.symbol_type.clone(),
                            language: chunk.language.clone(),
                            line_start: chunk.line_start,
                            line_end: chunk.line_end,
                            score: 0.0,
                            code: String::new(),
                            signature: chunk.signature.clone(),
                            match_sources: vec!["vector".into()],
                        };
                        self.state.vector_index.insert(vector, result);
                    }
                }

                *self.state.indexed.write() = true;
                *self.state.codebase_path.write() = Some(path.to_string());
                self.state.query_cache.invalidate();

                json!({
                    "status": "done",
                    "total_files": result.total_files,
                    "total_symbols": result.total_symbols,
                    "total_chunks": chunks.len(),
                    "graph_nodes": self.state.graph.node_count(),
                    "graph_relationships": self.state.graph.relationship_count(),
                    "vector_chunks": self.state.vector_index.len(),
                    "time_seconds": (result.time_seconds * 100.0).round() / 100.0,
                })
            }
            Err(e) => json!({"error": format!("Indexing failed: {}", e)}),
        }
    }

    fn handle_find_symbol(&self, args: &Value) -> Value {
        if !*self.state.indexed.read() {
            return json!({"error": "No codebase indexed. Run 'index' first."});
        }
        let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let exact = args.get("exact").and_then(|v| v.as_bool()).unwrap_or(true);
        if name.is_empty() {
            return json!({"error": "Missing required parameter: name"});
        }

        let cb = self.state.codebase_path.read().clone();
        let matches = self.state.graph.find_nodes_by_name(name, exact);
        if matches.is_empty() {
            return json!({"error": format!("Symbol '{}' not found.", name)});
        }

        let symbols: Vec<Value> = matches.iter().take(20).map(|node| {
            let fp = cb.as_ref().map(|b| std::path::Path::new(&node.location.file_path).strip_prefix(b).map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| node.location.file_path.clone())).unwrap_or_else(|| node.location.file_path.clone());
            json!({"name": node.name, "type": node.node_type.to_string(), "file": fp, "line": node.location.start_line, "language": node.language})
        }).collect();

        json!({"total": symbols.len(), "symbols": symbols})
    }

    fn tool_definitions() -> Vec<Tool> {
        let empty: Arc<serde_json::Map<String, Value>> = Arc::new(serde_json::Map::new());
        let mk = |schema_json: &str| -> Arc<serde_json::Map<String, Value>> {
            Arc::new(serde_json::from_str(schema_json).unwrap_or_default())
        };

        let path_schema =
            mk(r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}"#);
        let name_schema = mk(
            r#"{"type":"object","properties":{"name":{"type":"string"},"exact":{"type":"boolean"}},"required":["name"]}"#,
        );
        let sym_schema = mk(
            r#"{"type":"object","properties":{"symbol_name":{"type":"string"}},"required":["symbol_name"]}"#,
        );
        let query_schema = mk(
            r#"{"type":"object","properties":{"query":{"type":"string"},"limit":{"type":"integer"},"mode":{"type":"string"}},"required":["query"]}"#,
        );
        let impact_schema = mk(
            r#"{"type":"object","properties":{"symbol_name":{"type":"string"},"max_depth":{"type":"integer"}},"required":["symbol_name"]}"#,
        );
        let code_schema = mk(
            r#"{"type":"object","properties":{"operation":{"type":"string"},"file_path":{"type":"string"},"symbol_name":{"type":"string"},"symbols":{"type":"string"},"pattern":{"type":"string"},"path":{"type":"string"}},"required":["operation"]}"#,
        );
        let mem_schema = mk(
            r#"{"type":"object","properties":{"content":{"type":"string"},"memory_type":{"type":"string"},"tags":{"type":"string"},"ttl":{"type":"string"}},"required":["content"]}"#,
        );
        let recall_schema = mk(
            r#"{"type":"object","properties":{"query":{"type":"string"},"limit":{"type":"integer"},"memory_type":{"type":"string"},"tags":{"type":"string"}},"required":["query"]}"#,
        );
        let knowledge_schema = mk(
            r#"{"type":"object","properties":{"command":{"type":"string"},"name":{"type":"string"},"query":{"type":"string"},"value":{"type":"string"},"path":{"type":"string"}},"required":["command"]}"#,
        );
        let ref_schema = mk(
            r#"{"type":"object","properties":{"ref_id":{"type":"string"}},"required":["ref_id"]}"#,
        );
        let commit_schema = mk(
            r#"{"type":"object","properties":{"query":{"type":"string"},"limit":{"type":"integer"},"author":{"type":"string"}},"required":["query"]}"#,
        );
        let hist_schema = mk(r#"{"type":"object","properties":{"limit":{"type":"integer"}}}"#);

        vec![
            Tool::new("status", "Get server status", empty.clone()),
            Tool::new("health", "Health check", empty.clone()),
            Tool::new("index", "Index a codebase", path_schema.clone()),
            Tool::new(
                "search",
                "Semantic + keyword + graph hybrid search",
                query_schema,
            ),
            Tool::new("find_symbol", "Find a symbol definition", name_schema),
            Tool::new(
                "find_callers",
                "Who calls this function?",
                sym_schema.clone(),
            ),
            Tool::new(
                "find_callees",
                "What does this function call?",
                sym_schema.clone(),
            ),
            Tool::new("explain", "Full symbol explanation", sym_schema.clone()),
            Tool::new("impact", "What breaks if I change this?", impact_schema),
            Tool::new("overview", "Project structure summary", empty.clone()),
            Tool::new(
                "architecture",
                "Layers, entry points, hub symbols",
                empty.clone(),
            ),
            Tool::new("analyze", "Code smells and complexity", path_schema.clone()),
            Tool::new(
                "focus",
                "Low-token context slice for a file",
                path_schema.clone(),
            ),
            Tool::new(
                "dead_code",
                "Entry-point reachability analysis",
                empty.clone(),
            ),
            Tool::new(
                "circular_dependencies",
                "SCC-based circular deps",
                empty.clone(),
            ),
            Tool::new(
                "test_coverage_map",
                "Static test coverage map",
                empty.clone(),
            ),
            Tool::new(
                "code",
                "AST operations: symbols, patterns, rewrites",
                code_schema,
            ),
            Tool::new("remember", "Store a note or decision", mem_schema),
            Tool::new("recall", "Search memories by meaning", recall_schema),
            Tool::new(
                "forget",
                "Delete memories",
                mk(
                    r#"{"type":"object","properties":{"memory_id":{"type":"string"},"tags":{"type":"string"},"memory_type":{"type":"string"}}}"#,
                ),
            ),
            Tool::new("knowledge", "Index and search docs/notes", knowledge_schema),
            Tool::new(
                "compact",
                "Archive session content",
                mk(
                    r#"{"type":"object","properties":{"content":{"type":"string"}},"required":["content"]}"#,
                ),
            ),
            Tool::new(
                "session_snapshot",
                "Recover state after compaction",
                empty.clone(),
            ),
            Tool::new("restore", "Project re-entry summary", empty.clone()),
            Tool::new("retrieve", "Fetch sandboxed large output", ref_schema),
            Tool::new(
                "commit_search",
                "Semantic search over git history",
                commit_schema,
            ),
            Tool::new("commit_history", "Browse recent commits", hist_schema),
            Tool::new("repo_add", "Register another repo", path_schema.clone()),
            Tool::new("repo_remove", "Unregister a repo", path_schema),
            Tool::new("repo_status", "View all repos", empty.clone()),
            Tool::new("audit", "Packaged audit report", empty.clone()),
            Tool::new(
                "docs_bundle",
                "Generate docs bundle",
                mk(r#"{"type":"object","properties":{"output_dir":{"type":"string"}}}"#),
            ),
            Tool::new(
                "sidecar_export",
                "Generate .graph.* sidecars",
                mk(r#"{"type":"object","properties":{"path":{"type":"string"}}}"#),
            ),
            Tool::new("skill_prompt", "Print agent bootstrap block", empty.clone()),
            Tool::new(
                "introspect",
                "Look up Contextro docs",
                mk(r#"{"type":"object","properties":{"query":{"type":"string"}}}"#),
            ),
        ]
    }
}

impl Default for ContextroServer {
    fn default() -> Self {
        Self::new()
    }
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
            instructions: Some("Contextro: code intelligence MCP server. 35 tools for search, graph analysis, memory, and git.".into()),
        }
    }

    fn list_tools(
        &self,
        _request: PaginatedRequestParam,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        std::future::ready(Ok(ListToolsResult {
            tools: Self::tool_definitions(),
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

#[tokio::main]
async fn main() -> Result<()> {
    // Handle --version / -V before starting the server.
    let cli_args: Vec<String> = std::env::args().collect();
    if cli_args.iter().any(|a| a == "--version" || a == "-V") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    info!(
        "Starting Contextro MCP server v{}",
        env!("CARGO_PKG_VERSION")
    );

    update_check::spawn();

    let server = ContextroServer::new();
    let transport = get_settings().read().transport.clone();

    match transport.as_str() {
        "http" => {
            let (host, port) = {
                let settings = get_settings().read();
                (settings.http_host.clone(), settings.http_port)
            };
            info!("HTTP transport on {}:{}", host, port);
            http::serve_http(server, &host, port).await?;
        }
        _ => {
            let service = server
                .serve(rmcp::transport::stdio())
                .await
                .map_err(|e| anyhow::anyhow!("{:?}", e))?;
            service.waiting().await?;
        }
    }
    Ok(())
}
