//! Indexing pipeline: discover → parse → chunk → embed → store.

pub mod chunker;
pub mod embedding;
pub mod file_scanner;
pub mod pipeline;

pub use chunker::create_chunks;
pub use embedding::{embed, embed_batch};
pub use file_scanner::{diff_file_states, discover_files, hash_files, load_hashes, save_hashes};
pub use pipeline::{IndexResult, IndexingPipeline};
