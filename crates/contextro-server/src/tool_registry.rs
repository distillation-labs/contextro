//! Shared MCP tool registry construction for server transports.

use std::sync::Arc;

use contextro_tools::tool_manifest::{tool_docs, tool_docs_for_tier, ToolDoc, ToolTier};
use rmcp::model::Tool;
use serde_json::Value;

fn schema(schema_json: &str) -> Arc<serde_json::Map<String, Value>> {
    Arc::new(serde_json::from_str(schema_json).unwrap_or_default())
}

fn to_rmcp_tool(doc: &ToolDoc) -> Tool {
    Tool::new(doc.name, doc.description, schema(doc.schema_json))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn all_tool_definitions() -> Vec<Tool> {
    tool_docs().iter().map(to_rmcp_tool).collect()
}

pub(crate) fn configured_tool_definitions() -> Vec<Tool> {
    let mut docs = tool_docs_for_tier(ToolTier::configured());
    docs.sort_by(|a, b| a.name.cmp(b.name));
    docs.into_iter().map(to_rmcp_tool).collect()
}
