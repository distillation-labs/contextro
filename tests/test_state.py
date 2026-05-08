"""Tests for state.py."""

from pathlib import Path

import pytest

from contextro_mcp.state import get_state, reset_state


@pytest.fixture(autouse=True)
def clean_state(tmp_path, monkeypatch):
    """Reset global state and point storage at a fresh tmp dir before each test."""
    monkeypatch.setenv("CTX_STORAGE_DIR", str(tmp_path / ".contextro"))
    from contextro_mcp.config import reset_settings

    reset_settings()
    reset_state()
    yield
    reset_state()
    reset_settings()


def test_initial_state():
    state = get_state()
    assert not state.is_indexed
    assert state.codebase_path is None
    assert state.vector_engine is None
    assert state.graph_engine is None
    assert state.memory_store is None


def test_state_singleton():
    s1 = get_state()
    s2 = get_state()
    assert s1 is s2


def test_state_set_codebase():
    state = get_state()
    state.codebase_path = Path("/project")
    assert state.is_indexed
    assert state.codebase_path == Path("/project")


def test_state_engine_setters():
    state = get_state()
    state.vector_engine = "mock_vector"
    state.graph_engine = "mock_graph"
    state.memory_store = "mock_memory"
    assert state.vector_engine == "mock_vector"
    assert state.graph_engine == "mock_graph"
    assert state.memory_store == "mock_memory"


def test_reset_state():
    state = get_state()
    state.codebase_path = Path("/project")
    reset_state()
    state2 = get_state()
    assert not state2.is_indexed


def test_capture_index_snapshot():
    state = get_state()

    class MockVectorEngine:
        def count(self):
            return 7

    class MockBM25Engine:
        _fts_index_created = True

    class MockGraphEngine:
        def get_statistics(self):
            return {"total_nodes": 3, "total_relationships": 2}

    state.vector_engine = MockVectorEngine()
    state.bm25_engine = MockBM25Engine()
    state.graph_engine = MockGraphEngine()

    snapshot = state.capture_index_snapshot(commits_indexed=5)

    assert snapshot == {
        "vector_chunks": 7,
        "bm25_fts_ready": True,
        "graph": {"total_nodes": 3, "total_relationships": 2},
        "commits_indexed": 5,
    }
    assert state.index_snapshot == snapshot
