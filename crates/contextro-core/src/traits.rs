//! Abstract interfaces for Contextro components.
//!
//! These traits define the contracts that engines and parsers must fulfill,
//! enabling swappable implementations and testability.

use crate::errors::ContextroError;
use crate::models::{ParsedFile, SearchResult};

/// Abstract interface for code parsers.
pub trait Parser: Send + Sync {
    /// Check if this parser can handle the given file.
    fn can_parse(&self, filepath: &str) -> bool;

    /// Parse a source file and extract all symbols.
    fn parse_file(&self, filepath: &str) -> Result<ParsedFile, ContextroError>;

    /// Return supported file extensions (with leading dots).
    fn supported_extensions(&self) -> &[&str];
}

/// Abstract interface for search/storage engines.
pub trait Engine: Send + Sync {
    /// Add items to the engine.
    fn add(&self, items: Vec<serde_json::Value>) -> Result<(), ContextroError>;

    /// Search and return ranked results.
    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, ContextroError>;

    /// Delete items by ID.
    fn delete(&self, ids: &[String]) -> Result<(), ContextroError>;

    /// Return total number of items.
    fn count(&self) -> Result<usize, ContextroError>;

    /// Clear all items.
    fn clear(&self) -> Result<(), ContextroError>;
}

/// Trait for embedding services.
pub trait EmbeddingService: Send + Sync {
    /// Embed a single text.
    fn embed(&self, text: &str) -> Result<Vec<f32>, ContextroError>;

    /// Embed a batch of texts.
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, ContextroError>;

    /// Get the embedding dimensions.
    fn dimensions(&self) -> usize;

    /// Unload the model to free memory.
    fn unload(&self) -> Result<(), ContextroError>;
}
