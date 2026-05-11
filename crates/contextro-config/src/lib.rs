//! Configuration for Contextro with CTX_ env prefix.
//!
//! All settings can be overridden via environment variables prefixed with `CTX_`.
//! The settings singleton is initialized once and shared across the application.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use parking_lot::RwLock;
use sha2::{Digest, Sha256};

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
    let short_hash = hex::encode(&hash[..3]);
    let project_name = Path::new(codebase_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".into());
    let slug = format!("{}-{}", project_name, short_hash);
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".contextro")
        .join("projects")
        .join(slug)
}

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

mod dirs {
    use std::path::PathBuf;
    pub fn home_dir() -> Option<PathBuf> {
        std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok()
            .map(PathBuf::from)
    }
}

/// Contextro configuration. All settings can be overridden via CTX_ env vars.
#[derive(Debug, Clone)]
pub struct Settings {
    // Storage
    pub storage_dir: String,

    // Embedding
    pub embedding_model: String,
    pub embedding_device: String,
    pub embedding_batch_size: usize,

    // Indexing
    pub max_file_size_mb: usize,
    pub max_workers: Option<usize>,
    pub chunk_max_chars: usize,
    pub chunk_context_mode: String,
    pub chunk_context_path_depth: usize,
    pub index_file_batch_size: usize,
    pub skip_astgrep: bool,
    pub smart_chunk_relationships_enabled: bool,
    pub smart_chunk_file_context_enabled: bool,
    pub incremental_index_fast_path_enabled: bool,

    // Graph
    pub graph_max_depth: usize,

    // Search
    pub search_mode: String,
    pub reranker_model: String,
    pub fusion_weight_vector: f64,
    pub fusion_weight_bm25: f64,
    pub fusion_weight_graph: f64,
    pub relevance_threshold: f64,
    pub search_cache_max_size: usize,
    pub search_cache_similarity_threshold: f64,
    pub search_cache_ttl_seconds: f64,
    pub search_sandbox_threshold_tokens: usize,
    pub search_sandbox_max_entries: usize,
    pub search_sandbox_ttl_seconds: f64,
    pub search_preview_results: usize,
    pub search_preview_code_chars: usize,
    pub search_adaptive_result_count_enabled: bool,
    pub search_adaptive_high_confidence_limit: usize,
    pub search_adaptive_medium_confidence_limit: usize,
    pub search_code_budget_top_chars: usize,
    pub search_code_budget_second_chars: usize,
    pub search_code_budget_tail_chars: usize,
    pub search_prewarm_enabled: bool,

    // Memory
    pub max_memory_mb: usize,

    // Output
    pub output_format: String,

    // Logging
    pub log_level: String,

    // Security
    pub trust_remote_code: bool,
    pub default_permission_level: String,

    // Startup
    pub auto_warm_start: bool,

    // Audit
    pub audit_enabled: bool,

    // Rate limiting
    pub rate_limit_enabled: bool,
    pub rate_limit_default_rate: f64,
    pub rate_limit_default_burst: usize,

    // Git
    pub commit_history_enabled: bool,
    pub commit_history_limit: usize,
    pub commit_history_since: String,
    pub realtime_indexing_enabled: bool,
    pub branch_poll_interval_secs: f64,
    pub reindex_debounce_seconds: f64,
    pub file_watcher_enabled: bool,

    // Cross-repo
    pub cross_repo_enabled: bool,
    pub cross_repo_paths: String,

    // Transport
    pub transport: String,
    pub http_host: String,
    pub http_port: u16,
}

impl Default for Settings {
    fn default() -> Self {
        let storage_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".contextro")
            .to_string_lossy()
            .to_string();

        Self {
            storage_dir,
            embedding_model: "potion-code-16m".into(),
            embedding_device: "auto".into(),
            embedding_batch_size: 512,
            max_file_size_mb: 10,
            max_workers: None,
            chunk_max_chars: 4000,
            chunk_context_mode: "rich".into(),
            chunk_context_path_depth: 4,
            index_file_batch_size: 2000,
            skip_astgrep: true,
            smart_chunk_relationships_enabled: true,
            smart_chunk_file_context_enabled: true,
            incremental_index_fast_path_enabled: true,
            graph_max_depth: 10,
            search_mode: "hybrid".into(),
            reranker_model: "ms-marco-MiniLM-L-12-v2".into(),
            fusion_weight_vector: 0.5,
            fusion_weight_bm25: 0.3,
            fusion_weight_graph: 0.2,
            relevance_threshold: 0.40,
            search_cache_max_size: 128,
            search_cache_similarity_threshold: 0.92,
            search_cache_ttl_seconds: 300.0,
            search_sandbox_threshold_tokens: 1200,
            search_sandbox_max_entries: 100,
            search_sandbox_ttl_seconds: 600.0,
            search_preview_results: 4,
            search_preview_code_chars: 220,
            search_adaptive_result_count_enabled: true,
            search_adaptive_high_confidence_limit: 3,
            search_adaptive_medium_confidence_limit: 6,
            search_code_budget_top_chars: 320,
            search_code_budget_second_chars: 220,
            search_code_budget_tail_chars: 80,
            search_prewarm_enabled: true,
            max_memory_mb: 350,
            output_format: "json".into(),
            log_level: "INFO".into(),
            trust_remote_code: true,
            default_permission_level: "full".into(),
            auto_warm_start: false,
            audit_enabled: true,
            rate_limit_enabled: false,
            rate_limit_default_rate: 10.0,
            rate_limit_default_burst: 20,
            commit_history_enabled: true,
            commit_history_limit: 500,
            commit_history_since: String::new(),
            realtime_indexing_enabled: true,
            branch_poll_interval_secs: 2.0,
            reindex_debounce_seconds: 3.0,
            file_watcher_enabled: true,
            cross_repo_enabled: true,
            cross_repo_paths: String::new(),
            transport: "stdio".into(),
            http_host: "0.0.0.0".into(),
            http_port: 8000,
        }
    }
}

impl Settings {
    /// Build settings from environment variables with CTX_ prefix.
    pub fn from_env() -> Self {
        let mut s = Self::default();

        macro_rules! env_str {
            ($var:expr, $field:ident) => {
                if let Ok(v) = std::env::var($var) {
                    s.$field = v;
                }
            };
        }
        macro_rules! env_usize {
            ($var:expr, $field:ident) => {
                if let Ok(v) = std::env::var($var) {
                    if let Ok(n) = v.parse::<usize>() {
                        s.$field = n;
                    }
                }
            };
        }
        macro_rules! env_f64 {
            ($var:expr, $field:ident) => {
                if let Ok(v) = std::env::var($var) {
                    if let Ok(n) = v.parse::<f64>() {
                        s.$field = n;
                    }
                }
            };
        }
        macro_rules! env_bool {
            ($var:expr, $field:ident) => {
                if let Ok(v) = std::env::var($var) {
                    s.$field = matches!(v.to_lowercase().as_str(), "true" | "1" | "yes");
                }
            };
        }
        macro_rules! env_u16 {
            ($var:expr, $field:ident) => {
                if let Ok(v) = std::env::var($var) {
                    if let Ok(n) = v.parse::<u16>() {
                        s.$field = n;
                    }
                }
            };
        }

        env_str!("CTX_STORAGE_DIR", storage_dir);
        env_str!("CTX_EMBEDDING_MODEL", embedding_model);
        env_str!("CTX_EMBEDDING_DEVICE", embedding_device);
        env_usize!("CTX_EMBEDDING_BATCH_SIZE", embedding_batch_size);
        env_usize!("CTX_MAX_FILE_SIZE_MB", max_file_size_mb);
        env_usize!("CTX_CHUNK_MAX_CHARS", chunk_max_chars);
        env_str!("CTX_CHUNK_CONTEXT_MODE", chunk_context_mode);
        env_usize!("CTX_INDEX_FILE_BATCH_SIZE", index_file_batch_size);
        env_bool!("CTX_SKIP_ASTGREP", skip_astgrep);
        env_bool!(
            "CTX_SMART_CHUNK_RELATIONSHIPS_ENABLED",
            smart_chunk_relationships_enabled
        );
        env_bool!(
            "CTX_SMART_CHUNK_FILE_CONTEXT_ENABLED",
            smart_chunk_file_context_enabled
        );
        env_bool!(
            "CTX_INCREMENTAL_INDEX_FAST_PATH_ENABLED",
            incremental_index_fast_path_enabled
        );
        env_usize!("CTX_GRAPH_MAX_DEPTH", graph_max_depth);
        env_str!("CTX_SEARCH_MODE", search_mode);
        env_f64!("CTX_FUSION_WEIGHT_VECTOR", fusion_weight_vector);
        env_f64!("CTX_FUSION_WEIGHT_BM25", fusion_weight_bm25);
        env_f64!("CTX_FUSION_WEIGHT_GRAPH", fusion_weight_graph);
        env_f64!("CTX_RELEVANCE_THRESHOLD", relevance_threshold);
        env_usize!("CTX_SEARCH_CACHE_MAX_SIZE", search_cache_max_size);
        env_f64!(
            "CTX_SEARCH_CACHE_SIMILARITY_THRESHOLD",
            search_cache_similarity_threshold
        );
        env_f64!("CTX_SEARCH_CACHE_TTL_SECONDS", search_cache_ttl_seconds);
        env_usize!(
            "CTX_SEARCH_SANDBOX_THRESHOLD_TOKENS",
            search_sandbox_threshold_tokens
        );
        env_usize!("CTX_SEARCH_SANDBOX_MAX_ENTRIES", search_sandbox_max_entries);
        env_f64!("CTX_SEARCH_SANDBOX_TTL_SECONDS", search_sandbox_ttl_seconds);
        env_usize!("CTX_SEARCH_PREVIEW_RESULTS", search_preview_results);
        env_usize!("CTX_SEARCH_PREVIEW_CODE_CHARS", search_preview_code_chars);
        env_bool!(
            "CTX_SEARCH_ADAPTIVE_RESULT_COUNT_ENABLED",
            search_adaptive_result_count_enabled
        );
        env_usize!("CTX_MAX_MEMORY_MB", max_memory_mb);
        env_str!("CTX_OUTPUT_FORMAT", output_format);
        env_str!("CTX_LOG_LEVEL", log_level);
        env_bool!("CTX_TRUST_REMOTE_CODE", trust_remote_code);
        env_str!("CTX_PERMISSION_LEVEL", default_permission_level);
        env_bool!("CTX_AUTO_WARM_START", auto_warm_start);
        env_bool!("CTX_AUDIT_ENABLED", audit_enabled);
        env_bool!("CTX_RATE_LIMIT_ENABLED", rate_limit_enabled);
        env_f64!("CTX_RATE_LIMIT_DEFAULT_RATE", rate_limit_default_rate);
        env_usize!("CTX_RATE_LIMIT_DEFAULT_BURST", rate_limit_default_burst);
        env_bool!("CTX_COMMIT_HISTORY_ENABLED", commit_history_enabled);
        env_usize!("CTX_COMMIT_HISTORY_LIMIT", commit_history_limit);
        env_str!("CTX_COMMIT_HISTORY_SINCE", commit_history_since);
        env_bool!("CTX_REALTIME_INDEXING_ENABLED", realtime_indexing_enabled);
        env_f64!("CTX_BRANCH_POLL_INTERVAL", branch_poll_interval_secs);
        env_f64!("CTX_REINDEX_DEBOUNCE_SECONDS", reindex_debounce_seconds);
        env_bool!("CTX_FILE_WATCHER_ENABLED", file_watcher_enabled);
        env_bool!("CTX_CROSS_REPO_ENABLED", cross_repo_enabled);
        env_str!("CTX_CROSS_REPO_PATHS", cross_repo_paths);
        env_str!("CTX_TRANSPORT", transport);
        env_str!("CTX_HTTP_HOST", http_host);
        env_u16!("CTX_HTTP_PORT", http_port);

        // Handle max_workers specially (Option<usize>)
        if let Ok(v) = std::env::var("CTX_MAX_WORKERS") {
            s.max_workers = v.parse::<usize>().ok();
        }

        s
    }

    /// Path to the storage directory.
    pub fn storage_path(&self) -> PathBuf {
        PathBuf::from(&self.storage_dir)
    }

    /// Path to the LanceDB directory.
    pub fn lancedb_path(&self) -> PathBuf {
        self.storage_path().join("lancedb")
    }

    /// Path to the graph SQLite database.
    pub fn graph_path(&self) -> PathBuf {
        self.storage_path().join("graph.db")
    }

    /// Path to the index metadata JSON file.
    pub fn metadata_path(&self) -> PathBuf {
        self.storage_path().join("index_metadata.json")
    }
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
        let s = Settings::from_env();
        assert_eq!(s.embedding_model, "jina-code");
        assert_eq!(s.http_port, 9000);
        std::env::remove_var("CTX_EMBEDDING_MODEL");
        std::env::remove_var("CTX_HTTP_PORT");
    }
}
