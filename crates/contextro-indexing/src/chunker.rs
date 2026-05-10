//! Symbol-to-CodeChunk conversion for storage.

use contextro_core::models::{CodeChunk, Symbol};
use contextro_config::get_settings;

/// Convert a list of symbols into embeddable code chunks.
pub fn create_chunks(symbols: &[Symbol]) -> Vec<CodeChunk> {
    symbols.iter().map(create_chunk).collect()
}

/// Convert a single symbol into a code chunk.
fn create_chunk(symbol: &Symbol) -> CodeChunk {
    let text = create_chunk_text(symbol);
    let id = CodeChunk::generate_id(&symbol.filepath, &symbol.name, symbol.line_start);

    CodeChunk {
        id,
        text,
        filepath: symbol.filepath.clone(),
        symbol_name: symbol.qualified_name(),
        symbol_type: symbol.symbol_type.to_string(),
        language: symbol.language.clone(),
        line_start: symbol.line_start,
        line_end: symbol.line_end,
        signature: symbol.signature.clone(),
        parent: symbol.parent.clone().unwrap_or_default(),
        docstring: symbol.docstring.chars().take(500).collect(),
        vector: vec![],
    }
}

/// Format a symbol into embeddable text using Contextual Retrieval pattern.
fn create_chunk_text(symbol: &Symbol) -> String {
    let settings = get_settings().read();
    let mut parts = Vec::new();

    // Context header
    let path_parts: Vec<&str> = symbol.filepath.split('/').collect();
    let depth = settings.chunk_context_path_depth;
    let short_path = if path_parts.len() > depth {
        path_parts[path_parts.len() - depth..].join("/")
    } else {
        symbol.filepath.clone()
    };

    parts.push(format!(
        "# {} in {}",
        symbol.qualified_name(),
        short_path
    ));
    parts.push(format!("{}: {}", symbol.symbol_type, symbol.qualified_name()));
    parts.push(String::new());

    // Docstring first for BM25 weight
    if !symbol.docstring.is_empty() {
        let doc: String = symbol.docstring.chars().take(500).collect();
        parts.push(doc);
        parts.push(String::new());
    }

    // Signature
    if !symbol.signature.is_empty() {
        parts.push(symbol.signature.clone());
        parts.push(String::new());
    }

    // Code snippet (truncated)
    if !symbol.code_snippet.is_empty() {
        let max_chars = settings.chunk_max_chars;
        let snippet: String = symbol.code_snippet.chars().take(max_chars).collect();
        parts.push(snippet);
        parts.push(String::new());
    }

    // Imports
    if !symbol.imports.is_empty() {
        parts.push(format!("Imports: {}", symbol.imports.join(", ")));
    }

    // Calls
    if !symbol.calls.is_empty() {
        parts.push(format!("Calls: {}", symbol.calls.join(", ")));
    }

    parts.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use contextro_core::models::SymbolType;

    #[test]
    fn test_create_chunk() {
        let sym = Symbol {
            name: "authenticate".into(),
            symbol_type: SymbolType::Function,
            filepath: "src/auth/service.py".into(),
            line_start: 10,
            line_end: 25,
            language: "python".into(),
            signature: "def authenticate(token: str) -> bool:".into(),
            docstring: "Verify a JWT token.".into(),
            parent: None,
            code_snippet: "def authenticate(token: str) -> bool:\n    return verify(token)".into(),
            imports: vec!["from jwt import verify".into()],
            calls: vec!["verify".into()],
        };

        let chunks = create_chunks(&[sym]);
        assert_eq!(chunks.len(), 1);
        let chunk = &chunks[0];
        assert!(chunk.text.contains("authenticate"));
        assert!(chunk.text.contains("Verify a JWT token"));
        assert!(chunk.text.contains("Calls: verify"));
        assert_eq!(chunk.id.len(), 16);
    }
}
