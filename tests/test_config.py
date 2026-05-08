"""Tests for config.py."""

import pytest

from contextro_mcp.config import Settings, get_settings, reset_settings


@pytest.fixture(autouse=True)
def clean_settings():
    reset_settings()
    yield
    reset_settings()


def test_default_settings():
    s = Settings()
    assert s.embedding_model == "potion-code-16m"
    assert s.embedding_device == "auto"
    assert s.embedding_batch_size == 512
    assert s.commit_include_diffs is False
    assert s.search_prewarm_enabled is True
    assert s.search_prewarm_reranker is True
    assert s.chunk_context_mode == "rich"
    assert s.smart_chunk_relationships_enabled is True
    assert s.status_use_cached_index_stats is True
    assert s.max_memory_mb == 350
    assert s.log_level == "INFO"


def test_storage_paths():
    from pathlib import Path

    s = Settings()
    # Default storage is ~/.contextro (absolute, works from any working dir)
    assert s.storage_path == Path.home() / ".contextro"
    assert "lancedb" in str(s.lancedb_path)
    assert "graph.db" in str(s.graph_path)


def test_env_override(monkeypatch):
    monkeypatch.setenv("CTX_EMBEDDING_MODEL", "bge-small-en")
    monkeypatch.setenv("CTX_EMBEDDING_BATCH_SIZE", "64")
    monkeypatch.setenv("CTX_SEARCH_PREWARM_ENABLED", "false")
    monkeypatch.setenv("CTX_STATUS_USE_CACHED_INDEX_STATS", "false")
    monkeypatch.setenv("CTX_CHUNK_CONTEXT_MODE", "minimal")
    monkeypatch.setenv("CTX_LOG_LEVEL", "DEBUG")
    s = Settings()
    assert s.embedding_model == "bge-small-en"
    assert s.embedding_batch_size == 64
    assert s.search_prewarm_enabled is False
    assert s.status_use_cached_index_stats is False
    assert s.chunk_context_mode == "minimal"
    assert s.log_level == "DEBUG"


def test_env_invalid_int_keeps_default(monkeypatch):
    monkeypatch.setenv("CTX_EMBEDDING_BATCH_SIZE", "not_a_number")
    s = Settings()
    assert s.embedding_batch_size == 512  # default


def test_singleton():
    s1 = get_settings()
    s2 = get_settings()
    assert s1 is s2


def test_reset_singleton():
    s1 = get_settings()
    reset_settings()
    s2 = get_settings()
    assert s1 is not s2
