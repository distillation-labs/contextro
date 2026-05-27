//! Real tree-sitter based multi-language code parser.
//!
//! Uses tree-sitter grammars to parse source files into ASTs, then extracts
//! symbols (functions, classes, methods, interfaces, enums, types) and call
//! relationships (function calls + JSX component usage).

use contextro_core::models::{ParsedFile, Symbol, SymbolType};
use contextro_core::traits::Parser;
use contextro_core::ContextroError;

use crate::language::get_language_for_file;

mod go;
mod heuristic;
mod java;
mod python;
mod rust;
mod shared;
mod ts_js;

use go::parse_go;
use heuristic::{extract_imports, parse_heuristic};
use java::parse_java;
use python::parse_python;
use rust::parse_rust;
use ts_js::parse_ts_js;

/// Production tree-sitter parser for symbol extraction.
pub struct TreeSitterParser;

impl TreeSitterParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TreeSitterParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser for TreeSitterParser {
    fn can_parse(&self, filepath: &str) -> bool {
        get_language_for_file(filepath).is_some()
    }

    fn parse_file(&self, filepath: &str) -> Result<ParsedFile, ContextroError> {
        let language = get_language_for_file(filepath)
            .ok_or_else(|| ContextroError::parse(format!("Unsupported file: {}", filepath)))?;

        let content = std::fs::read_to_string(filepath)
            .map_err(|e| ContextroError::parse(format!("Failed to read {}: {}", filepath, e)))?;

        let start = std::time::Instant::now();

        let symbols = match language {
            "typescript" | "javascript" => parse_ts_js(&content, filepath, language),
            "rust" => parse_rust(&content, filepath),
            "python" => parse_python(&content, filepath),
            "go" => parse_go(&content, filepath),
            "java" => parse_java(&content, filepath),
            _ => parse_heuristic(&content, filepath, language),
        };

        let imports = extract_imports(&content, language);
        let parse_time_ms = start.elapsed().as_secs_f64() * 1000.0;

        Ok(ParsedFile {
            filepath: filepath.to_string(),
            language: language.to_string(),
            symbols,
            imports,
            parse_time_ms,
            error: None,
        })
    }

    fn supported_extensions(&self) -> &[&str] {
        &[
            ".py", ".js", ".ts", ".rs", ".go", ".java", ".c", ".cpp", ".rb",
        ]
    }
}

#[cfg(test)]
mod tests;
