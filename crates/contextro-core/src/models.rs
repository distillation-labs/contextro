//! Core data models for Contextro.
//!
//! All models are immutable after construction (no `&mut self` methods)
//! and derive Serialize/Deserialize for persistence and transport.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ─── Symbol Types ────────────────────────────────────────────────────────────

/// Code symbol classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolType {
    Function,
    Class,
    Method,
    Variable,
}

impl std::fmt::Display for SymbolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Function => write!(f, "function"),
            Self::Class => write!(f, "class"),
            Self::Method => write!(f, "method"),
            Self::Variable => write!(f, "variable"),
        }
    }
}

/// A single code symbol extracted from a source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub symbol_type: SymbolType,
    pub filepath: String,
    pub line_start: u32,
    pub line_end: u32,
    pub language: String,
    pub signature: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub docstring: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub code_snippet: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calls: Vec<String>,
}

impl Symbol {
    /// Qualified name: `Parent.name` for methods, `name` otherwise.
    pub fn qualified_name(&self) -> String {
        match &self.parent {
            Some(parent) => format!("{}.{}", parent, self.name),
            None => self.name.clone(),
        }
    }

    /// Number of lines this symbol spans.
    pub fn line_count(&self) -> u32 {
        self.line_end.saturating_sub(self.line_start) + 1
    }
}

// ─── Parsed File ─────────────────────────────────────────────────────────────

/// Result of parsing a single source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedFile {
    pub filepath: String,
    pub language: String,
    #[serde(default)]
    pub symbols: Vec<Symbol>,
    #[serde(default)]
    pub imports: Vec<String>,
    #[serde(default)]
    pub parse_time_ms: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ParsedFile {
    pub fn is_successful(&self) -> bool {
        self.error.is_none()
    }

    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }

    pub fn symbols_by_type(&self, symbol_type: SymbolType) -> Vec<&Symbol> {
        self.symbols.iter().filter(|s| s.symbol_type == symbol_type).collect()
    }
}

// ─── Code Chunk ──────────────────────────────────────────────────────────────

/// A chunk of code ready for embedding and storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChunk {
    pub id: String,
    pub text: String,
    pub filepath: String,
    pub symbol_name: String,
    pub symbol_type: String,
    pub language: String,
    pub line_start: u32,
    pub line_end: u32,
    pub signature: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub parent: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub docstring: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vector: Vec<f32>,
}

impl CodeChunk {
    /// Generate a deterministic chunk ID from filepath, name, and line.
    pub fn generate_id(filepath: &str, name: &str, line_start: u32) -> String {
        let key = format!("{}:{}:{}", filepath, name, line_start);
        let hash = Sha256::digest(key.as_bytes());
        hex::encode(&hash[..8])
    }
}

// We need hex encoding for the chunk ID
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

// ─── Codebase Index ──────────────────────────────────────────────────────────

/// Complete index metadata for a codebase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodebaseIndex {
    pub root_path: String,
    pub total_files: usize,
    pub total_symbols: usize,
    pub total_chunks: usize,
    pub indexed_at: String,
}

// ─── Search Result ───────────────────────────────────────────────────────────

/// A single search result with relevance scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub filepath: String,
    pub symbol_name: String,
    pub symbol_type: String,
    pub language: String,
    pub line_start: u32,
    pub line_end: u32,
    pub score: f64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub code: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub match_sources: Vec<String>,
}

// ─── Memory ──────────────────────────────────────────────────────────────────

/// Memory entry types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    Conversation,
    Status,
    Decision,
    Preference,
    Doc,
    Note,
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conversation => write!(f, "conversation"),
            Self::Status => write!(f, "status"),
            Self::Decision => write!(f, "decision"),
            Self::Preference => write!(f, "preference"),
            Self::Doc => write!(f, "doc"),
            Self::Note => write!(f, "note"),
        }
    }
}

/// Valid TTL values for memory entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryTtl {
    Session,
    Day,
    Week,
    Month,
    Permanent,
}

impl Default for MemoryTtl {
    fn default() -> Self {
        Self::Permanent
    }
}

/// A semantic memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub content: String,
    pub memory_type: MemoryType,
    pub project: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: String,
    pub accessed_at: String,
    #[serde(default)]
    pub ttl: MemoryTtl,
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_source() -> String {
    "user".into()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_qualified_name() {
        let sym = Symbol {
            name: "authenticate".into(),
            symbol_type: SymbolType::Method,
            filepath: "src/auth.py".into(),
            line_start: 10,
            line_end: 25,
            language: "python".into(),
            signature: "def authenticate(self, token: str) -> bool:".into(),
            docstring: String::new(),
            parent: Some("AuthService".into()),
            code_snippet: String::new(),
            imports: vec![],
            calls: vec![],
        };
        assert_eq!(sym.qualified_name(), "AuthService.authenticate");
        assert_eq!(sym.line_count(), 16);
    }

    #[test]
    fn test_chunk_id_deterministic() {
        let id1 = CodeChunk::generate_id("src/main.py", "hello", 1);
        let id2 = CodeChunk::generate_id("src/main.py", "hello", 1);
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 16); // 8 bytes = 16 hex chars
    }

    #[test]
    fn test_symbol_type_display() {
        assert_eq!(SymbolType::Function.to_string(), "function");
        assert_eq!(SymbolType::Class.to_string(), "class");
    }

    #[test]
    fn test_parsed_file_success() {
        let pf = ParsedFile {
            filepath: "test.py".into(),
            language: "python".into(),
            symbols: vec![],
            imports: vec![],
            parse_time_ms: 1.5,
            error: None,
        };
        assert!(pf.is_successful());
        assert_eq!(pf.symbol_count(), 0);
    }
}
