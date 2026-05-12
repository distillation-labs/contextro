//! Embedding service using model2vec-rs for fast static embeddings.
//!
//! Loads potion-base-16M from HuggingFace and provides
//! embed/embed_batch operations for vector search.

use std::sync::OnceLock;

use model2vec::Model2Vec;
use parking_lot::RwLock;
use tracing::{error, info, warn};

static MODEL: OnceLock<RwLock<Option<Model2Vec>>> = OnceLock::new();

/// Default model to use for embeddings.
const DEFAULT_MODEL: &str = "minishlab/potion-base-16M";

/// Get or initialize the embedding model.
fn get_model() -> &'static RwLock<Option<Model2Vec>> {
    MODEL.get_or_init(|| {
        info!("Loading embedding model: {}", DEFAULT_MODEL);
        match Model2Vec::from_pretrained(DEFAULT_MODEL, None, None) {
            Ok(model) => {
                info!("Embedding model loaded successfully");
                RwLock::new(Some(model))
            }
            Err(e) => {
                error!(
                    "Failed to load embedding model '{}': {}. Vector search disabled.",
                    DEFAULT_MODEL, e
                );
                RwLock::new(None)
            }
        }
    })
}

/// Embed a single text string. Returns None if model not available.
pub fn embed(text: &str) -> Option<Vec<f32>> {
    let lock = get_model().read();
    let model = lock.as_ref()?;
    let texts = [text];
    match model.encode(texts) {
        Ok(embeddings) => {
            let row = embeddings.row(0);
            Some(row.to_vec())
        }
        Err(e) => {
            warn!("Embedding failed: {}", e);
            None
        }
    }
}

/// Embed a batch of texts. Returns None if model not available.
pub fn embed_batch(texts: &[&str]) -> Option<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Some(vec![]);
    }
    let lock = get_model().read();
    let model = lock.as_ref()?;
    match model.encode(texts) {
        Ok(embeddings) => {
            let mut results = Vec::with_capacity(texts.len());
            for i in 0..embeddings.nrows() {
                results.push(embeddings.row(i).to_vec());
            }
            Some(results)
        }
        Err(e) => {
            warn!("Batch embedding failed: {}", e);
            None
        }
    }
}

/// Get the embedding dimensions (0 if model not loaded).
pub fn dimensions() -> usize {
    let lock = get_model().read();
    match lock.as_ref() {
        Some(model) => {
            // Embed a single word to get dimensions
            match model.encode(["test"]) {
                Ok(emb) => emb.ncols(),
                Err(_) => 0,
            }
        }
        None => 0,
    }
}

/// Check if the embedding model is available.
pub fn is_available() -> bool {
    get_model().read().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embed_single() {
        if !is_available() {
            eprintln!("Skipping: model not available (needs download)");
            return;
        }
        let vec = embed("hello world").unwrap();
        assert!(!vec.is_empty());
        assert!(vec.len() > 100); // Should be 256+ dims
    }

    #[test]
    fn test_embed_batch() {
        if !is_available() {
            eprintln!("Skipping: model not available");
            return;
        }
        let vecs = embed_batch(&["hello", "world", "rust"]).unwrap();
        assert_eq!(vecs.len(), 3);
        assert_eq!(vecs[0].len(), vecs[1].len());
    }
}
