use super::*;
use contextro_core::graph::{UniversalLocation, UniversalNode};
use contextro_core::models::CodeChunk;
use contextro_core::NodeType;
use contextro_engines::bm25::Bm25Engine;
use contextro_engines::cache::QueryCache;
use contextro_engines::graph::CodeGraph;
use contextro_engines::vector::VectorIndex;

fn make_result(id: &str, score: f64, sources: &[&str]) -> SearchResult {
    SearchResult {
        id: id.into(),
        filepath: format!("{id}.rs"),
        symbol_name: id.into(),
        symbol_type: "function".into(),
        language: "rust".into(),
        line_start: 1,
        line_end: 1,
        score,
        code: String::new(),
        signature: String::new(),
        match_sources: sources.iter().map(|source| (*source).to_string()).collect(),
    }
}

fn make_named_result(
    id: &str,
    symbol_name: &str,
    filepath: &str,
    score: f64,
    sources: &[&str],
) -> SearchResult {
    SearchResult {
        id: id.into(),
        filepath: filepath.into(),
        symbol_name: symbol_name.into(),
        symbol_type: "function".into(),
        language: "rust".into(),
        line_start: 1,
        line_end: 1,
        score,
        code: String::new(),
        signature: "pub fn sample()".into(),
        match_sources: sources.iter().map(|source| (*source).to_string()).collect(),
    }
}

fn make_named_result_with_language(
    id: &str,
    symbol_name: &str,
    filepath: &str,
    language: &str,
    score: f64,
    sources: &[&str],
) -> SearchResult {
    let mut result = make_named_result(id, symbol_name, filepath, score, sources);
    result.language = language.into();
    result
}

fn make_chunk(id: &str, text: &str, name: &str, filepath: &str) -> CodeChunk {
    CodeChunk {
        id: id.into(),
        text: text.into(),
        filepath: filepath.into(),
        symbol_name: name.into(),
        symbol_type: "function".into(),
        language: "rust".into(),
        line_start: 1,
        line_end: 10,
        signature: format!("pub fn {name}()"),
        parent: String::new(),
        docstring: String::new(),
        vector: vec![],
    }
}

fn make_graph_node(id: &str, name: &str, filepath: &str, language: &str) -> UniversalNode {
    UniversalNode {
        id: id.into(),
        name: name.into(),
        node_type: NodeType::Function,
        location: UniversalLocation {
            file_path: filepath.into(),
            start_line: 1,
            end_line: 10,
            start_column: 0,
            end_column: 0,
            language: language.into(),
        },
        language: language.into(),
        ..Default::default()
    }
}

mod exact_paths;
mod rerank_advanced;
mod rerank_core;
mod search_flow;
