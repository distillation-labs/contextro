//! Error types for Contextro.

use thiserror::Error;

/// Top-level error type for all Contextro operations.
#[derive(Debug, Error)]
pub enum ContextroError {
    #[error("parse error: {0}")]
    Parse(String),

    #[error("indexing error: {0}")]
    Indexing(String),

    #[error("embedding error: {0}")]
    Embedding(String),

    #[error("search error: {0}")]
    Search(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("graph error: {0}")]
    Graph(String),

    #[error("memory error: {0}")]
    Memory(String),

    #[error("fusion error: {0}")]
    Fusion(String),

    #[error("git error: {0}")]
    Git(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("rate limited: retry after {retry_after_secs:.2}s")]
    RateLimited { retry_after_secs: f64 },

    #[error("not indexed: {0}")]
    NotIndexed(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("internal error: {0}")]
    Internal(String),
}

impl ContextroError {
    pub fn parse(msg: impl Into<String>) -> Self {
        Self::Parse(msg.into())
    }

    pub fn indexing(msg: impl Into<String>) -> Self {
        Self::Indexing(msg.into())
    }

    pub fn embedding(msg: impl Into<String>) -> Self {
        Self::Embedding(msg.into())
    }

    pub fn search(msg: impl Into<String>) -> Self {
        Self::Search(msg.into())
    }

    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn not_indexed() -> Self {
        Self::NotIndexed("No codebase indexed. Run 'index' first.".into())
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }
}
