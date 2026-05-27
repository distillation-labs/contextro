use std::path::PathBuf;

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
        let storage_dir = super::home_dir()
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
        super::env_loader::settings_from_env()
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
