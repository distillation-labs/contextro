//! Indexing pipeline: discover → parse → chunk → embed → store.

pub mod chunker;
pub mod embedding;
pub mod file_scanner;
pub mod pipeline;

pub use chunker::create_chunks;
pub use embedding::{embed, embed_batch};
pub use file_scanner::discover_files;
pub use pipeline::{IndexResult, IndexingPipeline};
