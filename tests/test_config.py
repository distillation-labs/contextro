"""Tests for config.py."""

import pytest

from contextia_mcp.config import Settings, get_settings, reset_settings


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
    assert s.max_memory_mb == 350
    assert s.log_level == "INFO"


def test_storage_paths():
    from pathlib import Path
    s = Settings()
    # Default storage is ~/.contextia (absolute, works from any working dir)
    assert s.storage_path == Path.home() / ".contextia"
    assert "lancedb" in str(s.lancedb_path)
    assert "graph.db" in str(s.graph_path)


def test_env_override(monkeypatch):
    monkeypatch.setenv("CTX_EMBEDDING_MODEL", "bge-small-en")
    monkeypatch.setenv("CTX_EMBEDDING_BATCH_SIZE", "64")
    monkeypatch.setenv("CTX_LOG_LEVEL", "DEBUG")
    s = Settings()
    assert s.embedding_model == "bge-small-en"
    assert s.embedding_batch_size == 64
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
