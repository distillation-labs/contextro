//! Contextro MCP server binary — single compiled Rust binary.

use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use contextro_core::models::SearchResult;
use rmcp::model::*;
use rmcp::Error as McpError;
use rmcp::{ServerHandler, ServiceExt};
use serde_json::{json, Value};
use tracing::info;

use contextro_config::get_settings;

#[path = "main/dispatch.rs"]
mod dispatch;
#[path = "http.rs"]
mod http;
#[path = "main/indexing.rs"]
mod indexing;
#[path = "repo_snapshot.rs"]
mod repo_snapshot;
#[path = "main/response_utils.rs"]
mod response_utils;
#[path = "main/scopes.rs"]
mod scopes;
#[path = "state.rs"]
mod state;
#[path = "main/support.rs"]
mod support;
#[path = "main/symbols.rs"]
mod symbols;
#[path = "tool_registry.rs"]
mod tool_registry;
#[path = "update_check.rs"]
mod update_check;
use state::{AppState, RepoScopeSnapshot};

#[cfg(test)]
use response_utils::{format_response, strip_response_paths, take_chars};
use support::normalize_repo_dir;
#[cfg(test)]
use support::resolve_refactor_targets;

/// The Contextro MCP server.
#[derive(Clone)]
pub struct ContextroServer {
    state: Arc<AppState>,
}

#[derive(Debug, Default)]
pub(crate) struct RestoreSnapshotMetrics {
    graph_ms: f64,
    bm25_ms: f64,
    vector_ms: f64,
    scope_ms: f64,
    total_ms: f64,
}

fn round_ms(ms: f64) -> f64 {
    (ms * 10.0).round() / 10.0
}

fn set_ms_field(response: &mut Value, key: &str, ms: f64) {
    response[key] = json!(round_ms(ms));
}

impl ContextroServer {
    pub fn new() -> Self {
        Self::from_state(AppState::new())
    }

    fn from_state(state: AppState) -> Self {
        let server = Self {
            state: Arc::new(state),
        };
        server.restore_persisted_active_scope();
        server
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn with_settings(settings: contextro_config::Settings) -> Self {
        let state = AppState::from_settings(settings).expect("failed to initialize app state");
        Self::from_state(state)
    }

    fn can_skip_reindex(
        requested_path: &str,
        loaded_path: Option<&str>,
        indexed: bool,
        is_incremental: bool,
        changed_count: usize,
    ) -> bool {
        if !indexed || !is_incremental || changed_count != 0 {
            return false;
        }

        let Some(loaded_path) = loaded_path else {
            return false;
        };

        normalize_repo_dir(requested_path) == normalize_repo_dir(loaded_path)
    }
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;

#[tokio::main]
async fn main() -> Result<()> {
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
