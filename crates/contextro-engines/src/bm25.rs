//! Tantivy-based BM25 full-text search engine.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use contextro_core::models::{CodeChunk, SearchResult};
use parking_lot::RwLock;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, BoostQuery, Occur, Query, QueryParser, TermQuery};
use tantivy::schema::*;
use tantivy::tokenizer::TokenStream;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, Searcher, Term};
use tracing::warn;

#[derive(Clone)]
struct QueryVariant {
    text: String,
    weight: f64,
}

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
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return vec![];
        }

        let searcher = self.reader.searcher();
        let mut metadata: HashMap<String, SearchResult> = HashMap::new();
        let mut scores: HashMap<String, f64> = HashMap::new();
        let requested_limit = limit.max(1).min(50);
        let query_limit = limit.saturating_mul(2).clamp(requested_limit, 50);

        let primary_results = self.run_query(&searcher, trimmed, query_limit);
        merge_variant_results(&mut scores, &mut metadata, primary_results.clone(), 1.0);

        let query_terms = collect_query_terms(trimmed);
        if should_run_supplemental_variants(&query_terms, &primary_results, requested_limit) {
            for variant in build_supplemental_query_variants(&query_terms) {
                let results = self.run_query(&searcher, &variant.text, query_limit);
                merge_variant_results(&mut scores, &mut metadata, results, variant.weight);
            }
        }

        let mut merged: Vec<SearchResult> = scores
            .into_iter()
            .filter_map(|(id, score)| {
                let mut result = metadata.remove(&id)?;
                result.score = score.min(1.0);
                Some(result)
            })
            .collect();

        merged.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        merged.truncate(limit);
        merged
    }

    fn run_query(&self, searcher: &Searcher, query: &str, limit: usize) -> Vec<SearchResult> {
        let parsed = match self.build_query(query) {
            Ok(q) => q,
            Err(e) => {
                warn!("BM25 query parse failed for '{query}': {e}");
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
                    code: get_text(self.f_text),
                    signature: get_text(self.f_signature),
                    match_sources: vec!["bm25".into()],
                })
            })
            .collect()
    }

    fn build_query(&self, query: &str) -> tantivy::Result<Box<dyn Query>> {
        if let Some(fast_query) = self.build_plain_token_query(query)? {
            return Ok(fast_query);
        }

        let mut query_parser = QueryParser::for_index(
            &self.index,
            vec![self.f_text, self.f_symbol_name, self.f_signature],
        );
        query_parser.set_field_boost(self.f_symbol_name, 5.0);
        query_parser.set_field_boost(self.f_signature, 3.0);
        Ok(query_parser.parse_query(query)?)
    }

    fn build_plain_token_query(&self, query: &str) -> tantivy::Result<Option<Box<dyn Query>>> {
        if !is_plain_bm25_query(query) {
            return Ok(None);
        }

        let terms = self.tokenize_query_terms(query)?;
        if terms.len() < 2 {
            return Ok(None);
        }

        let mut subqueries: Vec<(Occur, Box<dyn Query>)> = Vec::with_capacity(terms.len() * 3);
        for term in terms {
            subqueries.push((
                Occur::Should,
                Box::new(TermQuery::new(
                    Term::from_field_text(self.f_text, &term),
                    IndexRecordOption::WithFreqs,
                )),
            ));
            subqueries.push((
                Occur::Should,
                Box::new(BoostQuery::new(
                    Box::new(TermQuery::new(
                        Term::from_field_text(self.f_symbol_name, &term),
                        IndexRecordOption::WithFreqs,
                    )),
                    5.0,
                )),
            ));
            subqueries.push((
                Occur::Should,
                Box::new(BoostQuery::new(
                    Box::new(TermQuery::new(
                        Term::from_field_text(self.f_signature, &term),
                        IndexRecordOption::WithFreqs,
                    )),
                    3.0,
                )),
            ));
        }

        Ok(Some(Box::new(BooleanQuery::new(subqueries))))
    }

    fn tokenize_query_terms(&self, query: &str) -> tantivy::Result<Vec<String>> {
        let mut tokenizer = self.index.tokenizer_for_field(self.f_text)?;
        let mut token_stream = tokenizer.token_stream(query);
        let mut terms = Vec::new();
        let mut seen = HashSet::new();

        while token_stream.advance() {
            let token = token_stream.token().text.clone();
            if seen.insert(token.clone()) {
                terms.push(token);
            }
        }

        Ok(terms)
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

fn merge_variant_results(
    scores: &mut HashMap<String, f64>,
    metadata: &mut HashMap<String, SearchResult>,
    results: Vec<SearchResult>,
    weight: f64,
) {
    for (rank, result) in results.into_iter().enumerate() {
        let contribution =
            weight * (result.score.clamp(0.0, 1.0) * 0.85 + (1.0 / (rank as f64 + 1.0)) * 0.15);
        *scores.entry(result.id.clone()).or_default() += contribution;
        metadata.entry(result.id.clone()).or_insert(result);
    }
}

fn collect_query_terms(query: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut terms = Vec::new();

    for token in query.split(|ch: char| !ch.is_ascii_alphanumeric()) {
        let token = token.trim().to_ascii_lowercase();
        if token.len() < 3 || is_bm25_stopword(&token) || !seen.insert(token.clone()) {
            continue;
        }

        terms.push(token);
    }

    terms
}

fn should_run_supplemental_variants(
    query_terms: &[String],
    primary_results: &[SearchResult],
    requested_limit: usize,
) -> bool {
    if query_terms.is_empty() {
        return false;
    }

    if primary_results.is_empty() {
        return true;
    }

    if query_terms.len() == 1 {
        return false;
    }

    let grounded_terms = count_grounded_query_terms(query_terms, primary_results);
    let strong_grounding = grounded_terms >= query_terms.len().min(3);
    let broad_coverage = grounded_terms * 5 >= query_terms.len() * 3;
    let enough_primary_hits = primary_results.len() >= requested_limit.min(3);

    !(strong_grounding || (broad_coverage && enough_primary_hits))
}

fn count_grounded_query_terms(query_terms: &[String], primary_results: &[SearchResult]) -> usize {
    let lexical_window = primary_results.len().min(3);
    let grounded_text = primary_results
        .iter()
        .take(lexical_window)
        .map(|result| {
            format!(
                "{}\n{}\n{}\n{}",
                result.symbol_name, result.signature, result.filepath, result.code
            )
            .to_ascii_lowercase()
        })
        .collect::<Vec<_>>()
        .join("\n");

    query_terms
        .iter()
        .filter(|term| grounded_text.contains(term.as_str()))
        .count()
}

fn build_supplemental_query_variants(query_terms: &[String]) -> Vec<QueryVariant> {
    let mut variants = Vec::new();
    let mut seen = HashSet::new();

    for token in query_terms {
        let token = token.as_str();

        if seen.insert(token.to_string()) {
            variants.push(QueryVariant {
                text: token.to_string(),
                weight: 0.18,
            });
        }

        if let Some(stemmed) = stem_bm25_token(&token) {
            if seen.insert(stemmed.clone()) {
                variants.push(QueryVariant {
                    text: stemmed,
                    weight: 0.24,
                });
            }
        }
    }

    variants
}

fn stem_bm25_token(token: &str) -> Option<String> {
    let stemmed = if token.ends_with("ing") && token.len() > 5 {
        restore_stemmed_root(&token[..token.len() - 3])
    } else if token.ends_with("ed") && token.len() > 4 {
        restore_stemmed_root(&token[..token.len() - 2])
    } else if token.ends_with("ers") && token.len() > 5 {
        token[..token.len() - 3].to_string()
    } else if token.ends_with("er") && token.len() > 4 {
        token[..token.len() - 2].to_string()
    } else if token.ends_with("es") && token.len() > 4 {
        token[..token.len() - 2].to_string()
    } else if token.ends_with('s') && token.len() > 4 {
        token[..token.len() - 1].to_string()
    } else {
        token.to_string()
    };

    (stemmed.len() >= 3 && stemmed != token).then_some(stemmed)
}

fn restore_stemmed_root(base: &str) -> String {
    if base.ends_with("ch") || base.ends_with("sh") || base.ends_with('v') || base.ends_with('c') {
        format!("{base}e")
    } else {
        base.to_string()
    }
}

fn is_bm25_stopword(token: &str) -> bool {
    matches!(
        token,
        "and"
            | "are"
            | "does"
            | "for"
            | "from"
            | "how"
            | "into"
            | "the"
            | "this"
            | "that"
            | "what"
            | "when"
            | "where"
            | "which"
            | "with"
            | "work"
            | "works"
    )
}

fn is_plain_bm25_query(query: &str) -> bool {
    !query.is_empty()
        && !query.chars().any(|ch| {
            matches!(
                ch,
                '"' | '\'' | ':' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '*' | '?' | '~'
                    | '+' | '-'
            )
        })
        && !query
            .split_ascii_whitespace()
            .any(|token| matches!(token, "AND" | "OR" | "NOT"))
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

    #[test]
    fn test_bm25_search_recovers_cache_from_caching_query() {
        let engine = Bm25Engine::new_in_memory();
        let mut cache_chunk = make_chunk(
            "cache",
            "Query cache stores search responses with TTL eviction and invalidation support.",
            "QueryCache",
        );
        cache_chunk.filepath = "crates/contextro-engines/src/cache.rs".into();
        cache_chunk.signature = "pub struct QueryCache".into();
        let other_chunk = make_chunk(
            "search",
            "Search routing decides whether to use vector or BM25 retrieval.",
            "handle_search",
        );

        engine.index_chunks(&[cache_chunk, other_chunk]);

        let results = engine.search("how does caching work", 5);

        assert!(!results.is_empty());
        assert_eq!(results[0].symbol_name, "QueryCache");
        assert!(results[0]
            .code
            .to_ascii_lowercase()
            .contains("ttl eviction"));
    }

    #[test]
    fn test_bm25_search_uses_concept_tokens_for_query_cache_eviction() {
        let engine = Bm25Engine::new_in_memory();
        let mut cache_chunk = make_chunk(
            "cache",
            "Query cache stores results with TTL eviction and invalidates expired entries.",
            "QueryCache",
        );
        cache_chunk.filepath = "crates/contextro-engines/src/cache.rs".into();
        cache_chunk.signature = "pub struct QueryCache".into();

        let mut fusion_chunk = make_chunk(
            "fusion",
            "Reciprocal rank fusion combines BM25 and vector results.",
            "ReciprocalRankFusion",
        );
        fusion_chunk.filepath = "crates/contextro-engines/src/fusion.rs".into();

        engine.index_chunks(&[cache_chunk, fusion_chunk]);

        let results = engine.search("how does the query cache work TTL eviction", 5);

        assert!(!results.is_empty());
        assert_eq!(results[0].symbol_name, "QueryCache");
    }

    #[test]
    fn test_bm25_search_handles_widened_limits_without_panicking() {
        let engine = Bm25Engine::new_in_memory();
        let mut cache_chunk = make_chunk(
            "cache",
            "Query cache stores search results with TTL eviction and cache invalidation.",
            "QueryCache",
        );
        cache_chunk.filepath = "crates/contextro-engines/src/cache.rs".into();
        cache_chunk.signature = "pub struct QueryCache".into();

        engine.index_chunks(&[cache_chunk]);

        let results = engine.search("how does caching work", 80);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].symbol_name, "QueryCache");
        assert_eq!(results[0].filepath, "crates/contextro-engines/src/cache.rs");
    }

    #[test]
    fn test_bm25_skips_supplemental_variants_when_primary_query_is_well_grounded() {
        let query_terms = collect_query_terms("how does query cache handle ttl eviction");
        let primary_results = vec![SearchResult {
            id: "cache".into(),
            filepath: "crates/contextro-engines/src/cache.rs".into(),
            symbol_name: "QueryCache".into(),
            symbol_type: "struct".into(),
            language: "rust".into(),
            line_start: 1,
            line_end: 10,
            score: 1.0,
            code: "Query cache handle keeps TTL eviction behavior stable.".into(),
            signature: "pub struct QueryCache".into(),
            match_sources: vec!["bm25".into()],
        }];

        assert!(!should_run_supplemental_variants(
            &query_terms,
            &primary_results,
            5,
        ));
    }

    #[test]
    fn test_plain_query_fast_path_detects_query_syntax() {
        assert!(is_plain_bm25_query("query cache eviction"));
        assert!(!is_plain_bm25_query("symbol_name:QueryCache"));
        assert!(!is_plain_bm25_query("query AND cache"));
        assert!(!is_plain_bm25_query("\"query cache\""));
    }
}
