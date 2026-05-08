"""Configuration for Contextro with CTX_ env prefix."""

import hashlib
import os
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional


def _default_storage_dir() -> str:
    """Return the default storage directory.

    If CTX_STORAGE_DIR is set, use it directly.
    Otherwise default to ~/.contextro — a single shared location.
    Per-project isolation is achieved by calling index() which stores
    the codebase_path in index_metadata.json for warm-start recovery.
    """
    return os.environ.get("CTX_STORAGE_DIR", str(Path.home() / ".contextro"))


def project_storage_dir(codebase_path: str) -> Path:
    """Return a project-specific storage directory under ~/.contextro/projects/.

    Each unique codebase path gets its own isolated index so switching
    between projects doesn't overwrite each other's data.

    Example:
        /Users/alice/platform  →  ~/.contextro/projects/platform-a3f2b1/
        /Users/alice/backend   →  ~/.contextro/projects/backend-9c4d2e/
    """
    path_hash = hashlib.sha256(codebase_path.encode()).hexdigest()[:6]
    project_name = Path(codebase_path).name
    slug = f"{project_name}-{path_hash}"
    return Path.home() / ".contextro" / "projects" / slug


@dataclass
class Settings:
    """Contextro configuration. All settings can be overridden via CTX_ env vars."""

    # Storage — default to ~/.contextro so it works regardless of working directory
    storage_dir: str = field(default_factory=_default_storage_dir)

    # Embedding model
    embedding_model: str = "potion-code-16m"
    embedding_device: str = "auto"
    embedding_batch_size: int = 512

    # Indexing
    max_file_size_mb: int = 10
    max_workers: Optional[int] = None
    chunk_max_chars: int = 4000
    chunk_context_mode: str = "rich"  # "minimal" or "rich"
    chunk_context_path_depth: int = 4
    index_file_batch_size: int = 2000  # files per streaming batch (larger = fewer LanceDB writes)
    skip_astgrep: bool = True  # Skip ast-grep for faster indexing (tree-sitter handles most cases)
    smart_chunk_relationships_enabled: bool = True
    smart_chunk_file_context_enabled: bool = True

    # Graph
    graph_max_depth: int = 10

    # Search
    search_mode: str = "hybrid"  # "hybrid", "vector", "bm25"
    reranker_model: str = "ms-marco-MiniLM-L-12-v2"
    fusion_weight_vector: float = 0.5
    fusion_weight_bm25: float = 0.3
    fusion_weight_graph: float = 0.2
    # Drop results below this fraction of the top score.
    relevance_threshold: float = 0.40
    search_cache_max_size: int = 128
    search_cache_similarity_threshold: float = 0.92
    search_cache_ttl_seconds: float = 300.0
    search_sandbox_threshold_tokens: int = 1200
    search_sandbox_max_entries: int = 100
    search_sandbox_ttl_seconds: float = 600.0
    search_preview_results: int = 4
    search_preview_code_chars: int = 220
    search_adaptive_result_count_enabled: bool = True
    search_adaptive_high_confidence_limit: int = 3
    search_adaptive_medium_confidence_limit: int = 6
    search_prewarm_enabled: bool = True
    search_prewarm_reranker: bool = True
    search_query_aware_compression: bool = True
    search_query_window_radius: int = 2
    search_code_budget_top_chars: int = 320
    search_code_budget_second_chars: int = 220
    search_code_budget_tail_chars: int = 80
    search_code_focus_min_chars: int = 60
    status_use_cached_index_stats: bool = True

    # Memory limits
    max_memory_mb: int = 350

    # Output format
    output_format: str = "json"  # "json" or "toon" (token-optimized)

    # Logging
    log_level: str = "INFO"
    log_format: str = "text"  # "text" or "json"

    # Security
    trust_remote_code: bool = True
    auth_mode: str = "none"  # "none", "local", "oauth" (oauth deferred)
    default_permission_level: str = "full"  # "read" or "full"

    # Startup / recovery
    auto_warm_start: bool = False

    # Audit
    audit_enabled: bool = True
    audit_log_file: str = ""  # empty = stderr

    # Rate limiting
    rate_limit_enabled: bool = False
    rate_limit_default_rate: float = 10.0  # requests per second
    rate_limit_default_burst: int = 20

    # Git / Commit History
    commit_history_enabled: bool = True
    commit_history_limit: int = 500  # max commits to index
    commit_history_since: str = ""  # e.g. "6 months ago", empty = no limit
    commit_include_diffs: bool = (
        False  # faster/lower-memory default; opt in for diff-aware commit chunks
    )

    # Branch-Aware Real-Time Indexing
    realtime_indexing_enabled: bool = True
    branch_poll_interval: float = 2.0  # seconds between HEAD polls
    reindex_debounce_seconds: float = 3.0  # min seconds between reindex triggers
    file_watcher_enabled: bool = True  # auto-start file watcher on index

    # Cross-Repo Context
    cross_repo_enabled: bool = True
    cross_repo_paths: str = ""  # comma-separated additional repo paths

    def __post_init__(self):
        """Override settings from CTX_ environment variables."""

        def _bool(v):
            return v.lower() in ("true", "1", "yes")

        env_map = {
            "CTX_STORAGE_DIR": ("storage_dir", str),
            "CTX_EMBEDDING_MODEL": ("embedding_model", str),
            "CTX_EMBEDDING_DEVICE": ("embedding_device", str),
            "CTX_EMBEDDING_BATCH_SIZE": ("embedding_batch_size", int),
            "CTX_MAX_FILE_SIZE_MB": ("max_file_size_mb", int),
            "CTX_MAX_WORKERS": ("max_workers", lambda v: int(v) if v else None),
            "CTX_CHUNK_MAX_CHARS": ("chunk_max_chars", int),
            "CTX_CHUNK_CONTEXT_MODE": ("chunk_context_mode", str),
            "CTX_CHUNK_CONTEXT_PATH_DEPTH": ("chunk_context_path_depth", int),
            "CTX_INDEX_FILE_BATCH_SIZE": ("index_file_batch_size", int),
            "CTX_SKIP_ASTGREP": ("skip_astgrep", _bool),
            "CTX_SMART_CHUNK_RELATIONSHIPS_ENABLED": (
                "smart_chunk_relationships_enabled",
                _bool,
            ),
            "CTX_SMART_CHUNK_FILE_CONTEXT_ENABLED": (
                "smart_chunk_file_context_enabled",
                _bool,
            ),
            "CTX_GRAPH_MAX_DEPTH": ("graph_max_depth", int),
            "CTX_SEARCH_MODE": ("search_mode", str),
            "CTX_RERANKER_MODEL": ("reranker_model", str),
            "CTX_FUSION_WEIGHT_VECTOR": ("fusion_weight_vector", float),
            "CTX_FUSION_WEIGHT_BM25": ("fusion_weight_bm25", float),
            "CTX_FUSION_WEIGHT_GRAPH": ("fusion_weight_graph", float),
            "CTX_RELEVANCE_THRESHOLD": ("relevance_threshold", float),
            "CTX_SEARCH_CACHE_MAX_SIZE": ("search_cache_max_size", int),
            "CTX_SEARCH_CACHE_SIMILARITY_THRESHOLD": (
                "search_cache_similarity_threshold",
                float,
            ),
            "CTX_SEARCH_CACHE_TTL_SECONDS": ("search_cache_ttl_seconds", float),
            "CTX_SEARCH_SANDBOX_THRESHOLD_TOKENS": ("search_sandbox_threshold_tokens", int),
            "CTX_SEARCH_SANDBOX_MAX_ENTRIES": ("search_sandbox_max_entries", int),
            "CTX_SEARCH_SANDBOX_TTL_SECONDS": ("search_sandbox_ttl_seconds", float),
            "CTX_SEARCH_PREVIEW_RESULTS": ("search_preview_results", int),
            "CTX_SEARCH_PREVIEW_CODE_CHARS": ("search_preview_code_chars", int),
            "CTX_SEARCH_ADAPTIVE_RESULT_COUNT_ENABLED": (
                "search_adaptive_result_count_enabled",
                _bool,
            ),
            "CTX_SEARCH_ADAPTIVE_HIGH_CONFIDENCE_LIMIT": (
                "search_adaptive_high_confidence_limit",
                int,
            ),
            "CTX_SEARCH_ADAPTIVE_MEDIUM_CONFIDENCE_LIMIT": (
                "search_adaptive_medium_confidence_limit",
                int,
            ),
            "CTX_SEARCH_PREWARM_ENABLED": ("search_prewarm_enabled", _bool),
            "CTX_SEARCH_PREWARM_RERANKER": ("search_prewarm_reranker", _bool),
            "CTX_SEARCH_QUERY_AWARE_COMPRESSION": ("search_query_aware_compression", _bool),
            "CTX_SEARCH_QUERY_WINDOW_RADIUS": ("search_query_window_radius", int),
            "CTX_SEARCH_CODE_BUDGET_TOP_CHARS": ("search_code_budget_top_chars", int),
            "CTX_SEARCH_CODE_BUDGET_SECOND_CHARS": (
                "search_code_budget_second_chars",
                int,
            ),
            "CTX_SEARCH_CODE_BUDGET_TAIL_CHARS": ("search_code_budget_tail_chars", int),
            "CTX_SEARCH_CODE_FOCUS_MIN_CHARS": ("search_code_focus_min_chars", int),
            "CTX_STATUS_USE_CACHED_INDEX_STATS": ("status_use_cached_index_stats", _bool),
            "CTX_MAX_MEMORY_MB": ("max_memory_mb", int),
            "CTX_OUTPUT_FORMAT": ("output_format", str),
            "CTX_LOG_LEVEL": ("log_level", str),
            "CTX_LOG_FORMAT": ("log_format", str),
            "CTX_TRUST_REMOTE_CODE": ("trust_remote_code", _bool),
            "CTX_AUTH_MODE": ("auth_mode", str),
            "CTX_PERMISSION_LEVEL": ("default_permission_level", str),
            "CTX_AUTO_WARM_START": ("auto_warm_start", _bool),
            "CTX_AUDIT_ENABLED": ("audit_enabled", _bool),
            "CTX_AUDIT_LOG_FILE": ("audit_log_file", str),
            "CTX_RATE_LIMIT_ENABLED": ("rate_limit_enabled", _bool),
            "CTX_RATE_LIMIT_DEFAULT_RATE": ("rate_limit_default_rate", float),
            "CTX_RATE_LIMIT_DEFAULT_BURST": ("rate_limit_default_burst", int),
            # Git / Commit History
            "CTX_COMMIT_HISTORY_ENABLED": ("commit_history_enabled", _bool),
            "CTX_COMMIT_HISTORY_LIMIT": ("commit_history_limit", int),
            "CTX_COMMIT_HISTORY_SINCE": ("commit_history_since", str),
            "CTX_COMMIT_INCLUDE_DIFFS": ("commit_include_diffs", _bool),
            # Branch-Aware Real-Time Indexing
            "CTX_REALTIME_INDEXING_ENABLED": ("realtime_indexing_enabled", _bool),
            "CTX_BRANCH_POLL_INTERVAL": ("branch_poll_interval", float),
            "CTX_REINDEX_DEBOUNCE_SECONDS": ("reindex_debounce_seconds", float),
            "CTX_FILE_WATCHER_ENABLED": ("file_watcher_enabled", _bool),
            # Cross-Repo Context
            "CTX_CROSS_REPO_ENABLED": ("cross_repo_enabled", _bool),
            "CTX_CROSS_REPO_PATHS": ("cross_repo_paths", str),
        }

        for env_key, (attr, converter) in env_map.items():
            value = os.environ.get(env_key)
            if value is not None:
                try:
                    setattr(self, attr, converter(value))
                except (ValueError, TypeError):
                    pass  # Keep default on conversion error

    @property
    def storage_path(self) -> Path:
        return Path(self.storage_dir)

    @property
    def lancedb_path(self) -> Path:
        return self.storage_path / "lancedb"

    @property
    def graph_path(self) -> Path:
        return self.storage_path / "graph.db"


# Singleton
_settings: Optional[Settings] = None


def get_settings() -> Settings:
    """Get or create singleton settings."""
    global _settings
    if _settings is None:
        _settings = Settings()
    return _settings


def reset_settings() -> None:
    """Reset settings (for testing)."""
    global _settings
    _settings = None
