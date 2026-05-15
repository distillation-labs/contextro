//! Symbol-to-CodeChunk conversion for storage.

use contextro_config::get_settings;
use contextro_core::models::{CodeChunk, Symbol};

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
    let max_chars = settings.chunk_max_chars;
    let mut parts = Vec::new();

    // Context header
    let path_parts: Vec<&str> = symbol.filepath.split('/').collect();
    let depth = settings.chunk_context_path_depth;
    let short_path = if path_parts.len() > depth {
        path_parts[path_parts.len() - depth..].join("/")
    } else {
        symbol.filepath.clone()
    };

    parts.push(format!("# {}", symbol.qualified_name()));
    parts.push(format!("Kind: {}", symbol.symbol_type));
    parts.push(format!("Symbol: {}", symbol.name));
    parts.push(format!("File: {}", short_path));
    parts.push(format!("Language: {}", symbol.language));
    if let Some(parent) = &symbol.parent {
        if !parent.is_empty() {
            parts.push(format!("Parent: {}", parent));
        }
    }
    let aliases = semantic_aliases(symbol);
    if !aliases.is_empty() {
        parts.push(format!("Keywords: {}", aliases.join(", ")));
    }

    // Summary stays near the top so embeddings and reranking see natural-language intent.
    if !symbol.docstring.is_empty() {
        parts.push(String::new());
        parts.push("Summary:".into());
        parts.push(truncate_chars(&symbol.docstring, 700));
    }

    if !symbol.signature.is_empty() {
        parts.push(String::new());
        parts.push("Signature:".into());
        parts.push(symbol.signature.clone());
    }

    if !symbol.imports.is_empty() {
        parts.push(String::new());
        parts.push(format!(
            "Imports: {}",
            truncate_joined(&symbol.imports, 12, 320)
        ));
    }

    if !symbol.calls.is_empty() {
        parts.push(format!(
            "Calls: {}",
            truncate_joined(&symbol.calls, 16, 320)
        ));
    }

    if !symbol.code_snippet.is_empty() {
        let snippet_budget = max_chars.saturating_div(3).clamp(240, 1200);
        parts.push(String::new());
        parts.push("Code:".into());
        parts.push(truncate_chars(&symbol.code_snippet, snippet_budget));
    }

    truncate_chars(&parts.join("\n").trim().to_string(), max_chars)
}

fn semantic_aliases(symbol: &Symbol) -> Vec<String> {
    let mut aliases = Vec::new();
    aliases.extend(split_identifier_tokens(&symbol.name));
    if let Some(parent) = &symbol.parent {
        aliases.extend(split_identifier_tokens(parent));
    }
    aliases.extend(split_path_tokens(&symbol.filepath));
    aliases.sort();
    aliases.dedup();
    aliases.retain(|token| token.len() >= 3);
    aliases.truncate(16);
    aliases
}

fn split_identifier_tokens(text: &str) -> Vec<String> {
    let mut spaced = String::with_capacity(text.len() * 2);
    let mut prev_was_lower_or_digit = false;

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && prev_was_lower_or_digit {
                spaced.push(' ');
            }
            spaced.push(ch.to_ascii_lowercase());
            prev_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else {
            spaced.push(' ');
            prev_was_lower_or_digit = false;
        }
    }

    spaced.split_whitespace().map(String::from).collect()
}

fn split_path_tokens(path: &str) -> Vec<String> {
    path.split(&['/', '.', '-', '_'][..])
        .flat_map(split_identifier_tokens)
        .collect()
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn truncate_joined(values: &[String], max_items: usize, max_chars: usize) -> String {
    truncate_chars(
        &values
            .iter()
            .take(max_items)
            .cloned()
            .collect::<Vec<_>>()
            .join(", "),
        max_chars,
    )
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

    #[test]
    fn test_create_chunk_text_adds_structured_context_and_bounds_size() {
        let large_doc = "Session archive persistence across restart. ".repeat(40);
        let large_code = "pub fn retrieve_archived_session() { restore(); }\n".repeat(200);
        let sym = Symbol {
            name: "retrieve_archived_session".into(),
            symbol_type: SymbolType::Function,
            filepath: "crates/contextro-tools/src/session.rs".into(),
            line_start: 10,
            line_end: 60,
            language: "rust".into(),
            signature: "pub fn retrieve_archived_session(ref_id: &str) -> Result<String>".into(),
            docstring: large_doc,
            parent: Some("ArchiveStore".into()),
            code_snippet: large_code,
            imports: vec!["use contextro_memory::archive::ArchiveStore;".into()],
            calls: vec!["restore".into(), "load_index".into()],
        };

        let chunk = create_chunks(&[sym]).pop().unwrap();
        let max_chars = get_settings().read().chunk_max_chars;

        assert!(chunk
            .text
            .contains("# ArchiveStore.retrieve_archived_session"));
        assert!(chunk.text.contains("Kind: function"));
        assert!(chunk.text.contains("Symbol: retrieve_archived_session"));
        assert!(chunk.text.contains("Keywords:"));
        assert!(chunk.text.contains("archive"));
        assert!(chunk.text.contains("retrieve"));
        assert!(chunk.text.contains("session"));
        assert!(chunk.text.contains("Summary:"));
        assert!(chunk.text.contains("Signature:"));
        assert!(chunk.text.contains("Calls: restore, load_index"));
        assert!(chunk.text.contains("Code:"));
        assert!(chunk.text.len() <= max_chars);
    }
}
