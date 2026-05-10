//! Code parsing with tree-sitter for Contextro.
//!
//! Extracts symbols, imports, and call relationships from source files.
//! Supports 15+ languages via tree-sitter grammars.

pub mod language;
pub mod treesitter;

pub use language::{get_language_for_file, get_supported_extensions};
pub use treesitter::TreeSitterParser;
