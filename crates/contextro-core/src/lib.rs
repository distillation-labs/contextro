//! Contextro Core — domain types, traits, and errors.
//!
//! This crate defines the foundational types used across all Contextro crates:
//! symbols, parsed files, code chunks, graph models, memory, and search results.

pub mod errors;
pub mod graph;
pub mod models;
pub mod traits;

pub use errors::ContextroError;
pub use graph::{NodeType, RelationshipType, UniversalGraph, UniversalLocation, UniversalNode, UniversalRelationship};
pub use models::{CodeChunk, CodebaseIndex, Memory, MemoryType, ParsedFile, SearchResult, Symbol, SymbolType};
pub use traits::{Engine, Parser};

/// Result type alias using [`ContextroError`].
pub type Result<T> = std::result::Result<T, ContextroError>;
