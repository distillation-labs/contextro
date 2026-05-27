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

mod catalog;

pub use catalog::{find_tool_doc, tool_docs, tool_docs_for_tier};

#[cfg(test)]
mod tests;
