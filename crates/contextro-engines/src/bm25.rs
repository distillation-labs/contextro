//! Tantivy-based BM25 full-text search engine.

use std::path::Path;
use std::sync::Arc;

use contextro_core::models::{CodeChunk, SearchResult};
use parking_lot::RwLock;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy};
use tracing::warn;

/// BM25 search engine backed by Tantivy.
pub struct Bm25Engine {
    index: Index,
    reader: IndexReader,
    writer: Arc<RwLock<IndexWriter>>,
    _schema: Schema,
    // Field handles
    f_id: Field,
    f_text: Field,
    f_filepath: Field,
    f_symbol_name: Field,
    f_symbol_type: Field,
    f_language: Field,
    f_line_start: Field,
    f_line_end: Field,
    f_signature: Field,
}

impl Bm25Engine {
    /// Create a new BM25 engine with an in-memory index.
    pub fn new_in_memory() -> Self {
        let schema = Self::build_schema();
        let index = Index::create_in_ram(schema.clone());
        Self::from_index(index, schema)
    }

    /// Create a new BM25 engine with a persistent index at the given path.
    pub fn new_persistent(path: &Path) -> Self {
        let schema = Self::build_schema();
        std::fs::create_dir_all(path).ok();
        let index = Index::create_in_dir(path, schema.clone())
            .unwrap_or_else(|_| Index::open_in_dir(path).expect("Failed to open tantivy index"));
        Self::from_index(index, schema)
    }

    fn build_schema() -> Schema {
        let mut builder = Schema::builder();
        builder.add_text_field("id", STRING | STORED);
        builder.add_text_field("text", TEXT | STORED);
        builder.add_text_field("filepath", STRING | STORED);
        builder.add_text_field("symbol_name", TEXT | STORED);
        builder.add_text_field("symbol_type", STRING | STORED);
        builder.add_text_field("language", STRING | STORED);
        builder.add_u64_field("line_start", INDEXED | STORED);
        builder.add_u64_field("line_end", INDEXED | STORED);
        builder.add_text_field("signature", TEXT | STORED);
        builder.build()
    }

    fn from_index(index: Index, schema: Schema) -> Self {
        let writer = index
            .writer(50_000_000)
            .expect("Failed to create index writer");
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .expect("Failed to create reader");

        Self {
            f_id: schema.get_field("id").unwrap(),
            f_text: schema.get_field("text").unwrap(),
            f_filepath: schema.get_field("filepath").unwrap(),
            f_symbol_name: schema.get_field("symbol_name").unwrap(),
            f_symbol_type: schema.get_field("symbol_type").unwrap(),
            f_language: schema.get_field("language").unwrap(),
            f_line_start: schema.get_field("line_start").unwrap(),
            f_line_end: schema.get_field("line_end").unwrap(),
            f_signature: schema.get_field("signature").unwrap(),
            index,
            reader,
            writer: Arc::new(RwLock::new(writer)),
            _schema: schema,
        }
    }

    /// Index a batch of code chunks.
    pub fn index_chunks(&self, chunks: &[CodeChunk]) {
        let mut writer = self.writer.write();
        for chunk in chunks {
            writer
                .add_document(doc!(
                    self.f_id => chunk.id.as_str(),
                    self.f_text => chunk.text.as_str(),
                    self.f_filepath => chunk.filepath.as_str(),
                    self.f_symbol_name => chunk.symbol_name.as_str(),
                    self.f_symbol_type => chunk.symbol_type.as_str(),
                    self.f_language => chunk.language.as_str(),
                    self.f_line_start => chunk.line_start as u64,
                    self.f_line_end => chunk.line_end as u64,
                    self.f_signature => chunk.signature.as_str(),
                ))
                .ok();
        }
        writer.commit().ok();
        drop(writer);
        self.reader.reload().ok();
    }

    /// Full-text BM25 search with field boosting.
    /// symbol_name is boosted 3x, signature 2x, text 1x for better precision.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let searcher = self.reader.searcher();
        let mut query_parser = QueryParser::for_index(
            &self.index,
            vec![self.f_text, self.f_symbol_name, self.f_signature],
        );
        query_parser.set_field_boost(self.f_symbol_name, 3.0);
        query_parser.set_field_boost(self.f_signature, 2.0);

        let parsed = match query_parser.parse_query(query) {
            Ok(q) => q,
            Err(e) => {
                warn!("BM25 query parse failed: {}", e);
                return vec![];
            }
        };

        let top_docs = match searcher.search(&parsed, &TopDocs::with_limit(limit)) {
            Ok(docs) => docs,
            Err(e) => {
                warn!("BM25 search failed: {}", e);
                return vec![];
            }
        };

        let max_score = top_docs.first().map(|(s, _)| *s).unwrap_or(1.0).max(0.001);

        top_docs
            .into_iter()
            .filter_map(|(score, addr)| {
                let doc: tantivy::TantivyDocument = searcher.doc(addr).ok()?;
                let get_text = |f: Field| -> String {
                    doc.get_first(f)
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                };
                let get_u64 =
                    |f: Field| -> u64 { doc.get_first(f).and_then(|v| v.as_u64()).unwrap_or(0) };

                Some(SearchResult {
                    id: get_text(self.f_id),
                    filepath: get_text(self.f_filepath),
                    symbol_name: get_text(self.f_symbol_name),
                    symbol_type: get_text(self.f_symbol_type),
                    language: get_text(self.f_language),
                    line_start: get_u64(self.f_line_start) as u32,
                    line_end: get_u64(self.f_line_end) as u32,
                    score: (score as f64) / (max_score as f64),
                    code: String::new(),
                    signature: get_text(self.f_signature),
                    match_sources: vec!["bm25".into()],
                })
            })
            .collect()
    }

    /// Delete all documents matching a filepath.
    pub fn delete_by_filepath(&self, filepath: &str) {
        let mut writer = self.writer.write();
        let term = tantivy::Term::from_field_text(self.f_filepath, filepath);
        writer.delete_term(term);
        writer.commit().ok();
        drop(writer);
        self.reader.reload().ok();
    }

    /// Clear the entire index.
    pub fn clear(&self) {
        let mut writer = self.writer.write();
        writer.delete_all_documents().ok();
        writer.commit().ok();
        drop(writer);
        self.reader.reload().ok();
    }

    /// Count total documents.
    pub fn count(&self) -> usize {
        let searcher = self.reader.searcher();
        searcher.num_docs() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chunk(id: &str, text: &str, name: &str) -> CodeChunk {
        CodeChunk {
            id: id.into(),
            text: text.into(),
            filepath: "src/main.py".into(),
            symbol_name: name.into(),
            symbol_type: "function".into(),
            language: "python".into(),
            line_start: 1,
            line_end: 10,
            signature: format!("def {}():", name),
            parent: String::new(),
            docstring: String::new(),
            vector: vec![],
        }
    }

    #[test]
    fn test_bm25_index_and_search() {
        let engine = Bm25Engine::new_in_memory();
        let chunks = vec![
            make_chunk(
                "c1",
                "authenticate user with JWT token verification",
                "authenticate",
            ),
            make_chunk("c2", "connect to database and run migrations", "connect_db"),
            make_chunk(
                "c3",
                "parse configuration from environment variables",
                "parse_config",
            ),
        ];

        engine.index_chunks(&chunks);
        assert_eq!(engine.count(), 3);

        let results = engine.search("authentication JWT", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].symbol_name, "authenticate");
    }

    #[test]
    fn test_bm25_delete_and_clear() {
        let engine = Bm25Engine::new_in_memory();
        engine.index_chunks(&[make_chunk("c1", "hello world", "hello")]);
        assert_eq!(engine.count(), 1);

        engine.clear();
        assert_eq!(engine.count(), 0);
    }
}
