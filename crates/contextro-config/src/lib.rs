//! Configuration for Contextro with CTX_ env prefix.
//!
//! All settings can be overridden via environment variables prefixed with `CTX_`.
//! The settings singleton is initialized once and shared across the application.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use parking_lot::RwLock;
use sha2::{Digest, Sha256};

mod env_loader;
mod settings;

pub use settings::Settings;

static SETTINGS: OnceLock<RwLock<Settings>> = OnceLock::new();

/// Get the global settings instance.
pub fn get_settings() -> &'static RwLock<Settings> {
    SETTINGS.get_or_init(|| RwLock::new(Settings::from_env()))
}

/// Reset settings (for testing).
pub fn reset_settings() {
    if let Some(lock) = SETTINGS.get() {
        *lock.write() = Settings::from_env();
    }
}

/// Return a project-specific storage directory under ~/.contextro/projects/.
pub fn project_storage_dir(codebase_path: &str) -> PathBuf {
    let hash = Sha256::digest(codebase_path.as_bytes());
    let short_hash = encode_hex(&hash[..3]);
    let project_name = Path::new(codebase_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".into());
    let slug = format!("{project_name}-{short_hash}");

    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".contextro")
        .join("projects")
        .join(slug)
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let s = Settings::default();
        assert_eq!(s.embedding_model, "potion-code-16m");
        assert_eq!(s.search_mode, "hybrid");
        assert_eq!(s.relevance_threshold, 0.40);
        assert_eq!(s.http_port, 8000);
    }

    #[test]
    fn test_project_storage_dir() {
        let dir = project_storage_dir("/Users/alice/platform");
        let dir_str = dir.to_string_lossy();
        assert!(dir_str.contains(".contextro/projects/platform-"));
    }

    #[test]
    fn test_env_override() {
        std::env::set_var("CTX_EMBEDDING_MODEL", "jina-code");
        std::env::set_var("CTX_HTTP_PORT", "9000");
        std::env::set_var("CTX_CHUNK_CONTEXT_PATH_DEPTH", "6");
        std::env::set_var("CTX_RERANKER_MODEL", "rerank-v2");
        std::env::set_var("CTX_SEARCH_ADAPTIVE_HIGH_CONFIDENCE_LIMIT", "4");
        std::env::set_var("CTX_SEARCH_ADAPTIVE_MEDIUM_CONFIDENCE_LIMIT", "7");
        std::env::set_var("CTX_SEARCH_CODE_BUDGET_TOP_CHARS", "512");
        std::env::set_var("CTX_SEARCH_CODE_BUDGET_SECOND_CHARS", "256");
        std::env::set_var("CTX_SEARCH_CODE_BUDGET_TAIL_CHARS", "96");
        std::env::set_var("CTX_SEARCH_PREWARM_ENABLED", "false");

        let s = Settings::from_env();
        assert_eq!(s.embedding_model, "jina-code");
        assert_eq!(s.http_port, 9000);
        assert_eq!(s.chunk_context_path_depth, 6);
        assert_eq!(s.reranker_model, "rerank-v2");
        assert_eq!(s.search_adaptive_high_confidence_limit, 4);
        assert_eq!(s.search_adaptive_medium_confidence_limit, 7);
        assert_eq!(s.search_code_budget_top_chars, 512);
        assert_eq!(s.search_code_budget_second_chars, 256);
        assert_eq!(s.search_code_budget_tail_chars, 96);
        assert!(!s.search_prewarm_enabled);

        std::env::remove_var("CTX_EMBEDDING_MODEL");
        std::env::remove_var("CTX_HTTP_PORT");
        std::env::remove_var("CTX_CHUNK_CONTEXT_PATH_DEPTH");
        std::env::remove_var("CTX_RERANKER_MODEL");
        std::env::remove_var("CTX_SEARCH_ADAPTIVE_HIGH_CONFIDENCE_LIMIT");
        std::env::remove_var("CTX_SEARCH_ADAPTIVE_MEDIUM_CONFIDENCE_LIMIT");
        std::env::remove_var("CTX_SEARCH_CODE_BUDGET_TOP_CHARS");
        std::env::remove_var("CTX_SEARCH_CODE_BUDGET_SECOND_CHARS");
        std::env::remove_var("CTX_SEARCH_CODE_BUDGET_TAIL_CHARS");
        std::env::remove_var("CTX_SEARCH_PREWARM_ENABLED");
    }
}
