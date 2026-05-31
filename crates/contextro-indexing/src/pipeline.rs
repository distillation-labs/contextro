//! Indexing pipeline: orchestrates discover → parse → chunk → embed → store.

use std::path::Path;
use std::time::Instant;

use contextro_config::Settings;
use contextro_core::models::Symbol;
use contextro_core::traits::Parser;
use contextro_core::ContextroError;
use contextro_parsing::TreeSitterParser;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::chunker::create_chunks;
use crate::file_scanner::discover_files;

/// Result of an indexing operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexResult {
    pub total_files: usize,
    pub total_symbols: usize,
    pub total_chunks: usize,
    pub parse_errors: usize,
    pub graph_nodes: usize,
    pub graph_relationships: usize,
    pub time_seconds: f64,
    pub files_added: usize,
    pub files_modified: usize,
    pub files_deleted: usize,
    #[serde(default)]
    pub discover_ms: f64,
    #[serde(default)]
    pub parse_ms: f64,
    #[serde(default)]
    pub chunk_ms: f64,
}

/// Orchestrates the full indexing flow.
pub struct IndexingPipeline {
    settings: Settings,
    parser: TreeSitterParser,
}

impl IndexingPipeline {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            parser: TreeSitterParser::new(),
        }
    }

    /// Full index of a codebase.
    pub fn index(
        &self,
        codebase_path: &Path,
    ) -> Result<
        (
            IndexResult,
            Vec<Symbol>,
            Vec<contextro_core::models::CodeChunk>,
        ),
        ContextroError,
    > {
        let start = Instant::now();

        // Step 1: Discover files
        let discover_start = Instant::now();
        let files = discover_files(codebase_path, &self.settings);
        let discover_ms = discover_start.elapsed().as_secs_f64() * 1000.0;
        info!("Discovered {} files in {:?}", files.len(), codebase_path);

        let start = Instant::now();

        if files.is_empty() {
            return Ok((
                IndexResult {
                    time_seconds: start.elapsed().as_secs_f64(),
                    discover_ms,
                    ..Default::default()
                },
                vec![],
                vec![],
            ));
        }

        // Step 2: Parse symbols in parallel
        let parse_start = Instant::now();
        let results: Vec<_> = files
            .par_iter()
            .map(|path| {
                let path_str = path.to_string_lossy().to_string();
                self.parser.parse_file(&path_str)
            })
            .collect();
        let parse_ms = parse_start.elapsed().as_secs_f64() * 1000.0;

        let mut all_symbols = Vec::new();
        let mut parse_errors = 0;

        for result in results {
            match result {
                Ok(parsed) => {
                    if parsed.is_successful() {
                        all_symbols.extend(parsed.symbols);
                    } else {
                        parse_errors += 1;
                    }
                }
                Err(_) => parse_errors += 1,
            }
        }

        // Step 3: Create chunks
        let chunk_start = Instant::now();
        let chunks = create_chunks(&all_symbols);
        let chunk_ms = chunk_start.elapsed().as_secs_f64() * 1000.0;

        let elapsed = start.elapsed().as_secs_f64();
        info!(
            "Indexed {} files, {} symbols, {} chunks in {:.1}s",
            files.len(),
            all_symbols.len(),
            chunks.len(),
            elapsed
        );

        let result = IndexResult {
            total_files: files.len(),
            total_symbols: all_symbols.len(),
            total_chunks: chunks.len(),
            parse_errors,
            time_seconds: elapsed,
            discover_ms,
            parse_ms,
            chunk_ms,
            ..Default::default()
        };

        Ok((result, all_symbols, chunks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_index_pipeline() {
        let tmp = std::env::temp_dir().join("ctx_test_pipeline");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("src")).unwrap();
        fs::write(
            tmp.join("src/main.py"),
            "def hello():\n    \"\"\"Say hello.\"\"\"\n    print(\"hello\")\n",
        )
        .unwrap();

        let settings = Settings::default();
        let pipeline = IndexingPipeline::new(settings);
        let (result, symbols, chunks) = pipeline.index(&tmp).unwrap();

        assert_eq!(result.total_files, 1);
        assert!(result.total_symbols >= 1);
        assert!(result.total_chunks >= 1);
        assert!(!symbols.is_empty());
        assert!(!chunks.is_empty());

        fs::remove_dir_all(tmp).ok();
    }
}
