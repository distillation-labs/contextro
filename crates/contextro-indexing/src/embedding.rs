//! Embedding service using model2vec-rs for fast static embeddings.
//!
//! Loads potion-code-16M from HuggingFace and provides
//! embed/embed_batch operations for vector search.

use std::sync::OnceLock;

use model2vec::Model2Vec;
use parking_lot::RwLock;
use tracing::{error, info, warn};

static MODEL: OnceLock<RwLock<Option<Model2Vec>>> = OnceLock::new();

/// Map short model key names to HuggingFace `owner/model_name` IDs.
fn resolve_hf_id(key: &str) -> &str {
    match key {
        "potion-code-16m" | "potion-code-16M" => "minishlab/potion-code-16M",
        "potion-base-8m"  | "potion-base-8M"  => "minishlab/potion-base-8M",
        "potion-base-32m" | "potion-base-32M" => "minishlab/potion-base-32M",
        other => other,
    }
}

/// Locate a model in the HuggingFace Hub local cache.
///
/// The cache layout is `~/.cache/huggingface/hub/models--{owner}--{name}/snapshots/{hash}/`.
/// `model2vec::Model2Vec::from_pretrained` expects a LOCAL directory path, not an HF model ID,
/// so we must resolve the cache path ourselves.
fn find_hf_cache_path(hf_id: &str) -> Option<String> {
    // "minishlab/potion-code-16M" → "models--minishlab--potion-code-16M"
    let cache_key = format!("models--{}", hf_id.replace('/', "--"));
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let snapshots = format!("{}/.cache/huggingface/hub/{}/snapshots", home, cache_key);

    // Pick the first snapshot directory that contains tokenizer.json
    std::fs::read_dir(&snapshots)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path().to_string_lossy().into_owned())
        .find(|p| std::path::Path::new(p).join("tokenizer.json").exists())
}

/// Get or initialize the embedding model.
/// Reads the model key from Settings (CTX_EMBEDDING_MODEL), resolves to HuggingFace ID,
/// then locates the locally-cached model directory to pass to model2vec.
fn get_model() -> &'static RwLock<Option<Model2Vec>> {
    MODEL.get_or_init(|| {
        let key = {
            let settings = contextro_config::get_settings().read();
            settings.embedding_model.clone()
        };
        let hf_id = resolve_hf_id(&key);

        // Resolve to a local path (HF hub cache), or treat key as a literal local path
        let model_path = find_hf_cache_path(hf_id)
            .or_else(|| {
                // Maybe the user set CTX_EMBEDDING_MODEL to an absolute local path
                if std::path::Path::new(hf_id).join("tokenizer.json").exists() {
                    Some(hf_id.to_string())
                } else {
                    None
                }
            });

        match model_path {
            Some(path) => {
                info!("Loading embedding model: {} from {}", hf_id, path);
                match Model2Vec::from_pretrained(&path, None, None) {
                    Ok(model) => {
                        info!("Embedding model loaded successfully ({} dims)", {
                            match model.encode(["test"]) {
                                Ok(e) => e.ncols(),
                                Err(_) => 0,
                            }
                        });
                        RwLock::new(Some(model))
                    }
                    Err(e) => {
                        error!("Failed to load embedding model from '{}': {}. Vector search disabled.", path, e);
                        RwLock::new(None)
                    }
                }
            }
            None => {
                error!(
                    "Embedding model '{}' (HF id: {}) not found in local cache (~/.cache/huggingface/hub). \
                     Run `huggingface-cli download {}` or set CTX_EMBEDDING_MODEL to a local path. \
                     Vector search disabled.",
                    key, hf_id, hf_id
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
