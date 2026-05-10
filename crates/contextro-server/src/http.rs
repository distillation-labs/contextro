//! HTTP transport for Docker/team deployments using axum.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{get, post};
use axum::Router;
use serde_json::{json, Value};
use tracing::info;

use crate::state::AppState;
use crate::ContextroServer;

/// Start the HTTP server on the configured host:port.
pub async fn serve_http(server: ContextroServer, host: &str, port: u16) -> anyhow::Result<()> {
    let state = server.state.clone();

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/mcp", post(mcp_handler))
        .with_state(HttpState { server });

    let addr = format!("{}:{}", host, port);
    info!("Contextro HTTP server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Clone)]
struct HttpState {
    server: ContextroServer,
}

async fn health_handler(State(state): State<HttpState>) -> Json<Value> {
    let uptime = state.server.state.started_at.elapsed().as_secs_f64();
    Json(json!({
        "status": "healthy",
        "uptime_seconds": (uptime * 10.0).round() / 10.0,
        "indexed": *state.server.state.indexed.read(),
    }))
}

async fn mcp_handler(
    State(state): State<HttpState>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let method = body.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let id = body.get("id").cloned().unwrap_or(Value::Null);

    let result = match method {
        "initialize" => {
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "contextro", "version": env!("CARGO_PKG_VERSION")},
            })
        }
        "tools/list" => {
            let tools: Vec<Value> = ContextroServer::tool_definitions().iter().map(|t| {
                json!({"name": t.name, "description": t.description, "inputSchema": t.schema_as_json_value()})
            }).collect();
            json!({"tools": tools})
        }
        "tools/call" => {
            let params = body.get("params").cloned().unwrap_or(Value::Null);
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(Value::Null);
            let call_result = state.server.dispatch(name, args);
            // Serialize the content as-is
            let content: Vec<Value> = call_result.content.iter().map(|c| {
                serde_json::to_value(c).unwrap_or(Value::Null)
            }).collect();
            json!({"content": content})
        }
        _ => json!({"error": format!("Unknown method: {}", method)}),
    };

    let response = json!({"jsonrpc": "2.0", "id": id, "result": result});
    (StatusCode::OK, Json(response))
}

// rmcp model types used via server dispatch
