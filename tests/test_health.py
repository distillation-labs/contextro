"""Tests for health endpoint and new exceptions (Phase 5f)."""

import asyncio
from pathlib import Path

import pytest

import contextro_mcp.server as server_module
from contextro_mcp.config import reset_settings
from contextro_mcp.core.exceptions import (
    AuthenticationError,
    AuthorizationError,
    ContextroException,
    RateLimitError,
)
from contextro_mcp.state import get_state, reset_state
from tests.conftest import _call_tool


@pytest.fixture(autouse=True)
def clean_state(tmp_path, monkeypatch):
    monkeypatch.setenv("CTX_STORAGE_DIR", str(tmp_path / ".contextro"))
    reset_settings()
    reset_state()
    server_module._index_job = {}
    yield
    reset_settings()
    reset_state()
    server_module._index_job = {}


def test_health_returns_status():
    """Health tool returns expected structure."""
    mcp = server_module.create_server()
    result = asyncio.run(_call_tool(mcp, "health"))
    assert result["status"] == "healthy"
    assert "uptime_seconds" in result
    assert "indexed" in result
    assert "engines" in result


def test_health_shows_engine_states():
    """Health shows all engine availability flags."""
    mcp = server_module.create_server()
    result = asyncio.run(_call_tool(mcp, "health"))
    engines = result["engines"]
    assert "vector" in engines
    assert "bm25" in engines
    assert "graph" in engines
    assert "memory" in engines
    assert engines["vector"] is False
    assert engines["graph"] is False


def test_health_uptime_positive():
    """Health uptime is a positive number."""
    mcp = server_module.create_server()
    result = asyncio.run(_call_tool(mcp, "health"))
    assert result["uptime_seconds"] >= 0


def test_health_works_before_indexing():
    """Health works even when nothing is indexed."""
    mcp = server_module.create_server()
    result = asyncio.run(_call_tool(mcp, "health"))
    assert result["indexed"] is False
    assert result["status"] == "healthy"


def test_status_hides_indexing_flag_once_state_is_indexed():
    mcp = server_module.create_server()
    state = get_state()
    state.codebase_path = Path("/repo")

    with server_module._index_job_lock:
        server_module._index_job = {"status": "indexing"}

    result = asyncio.run(_call_tool(mcp, "status"))

    assert result["indexed"] is True
    assert "indexing" not in result


def test_status_preserves_completed_job_hint_before_state_load():
    mcp = server_module.create_server()

    with server_module._index_job_lock:
        server_module._index_job = {"status": "done", "result": {}}

    result = asyncio.run(_call_tool(mcp, "status"))

    assert result["indexed"] is False
    assert result["hint"] == "Index job completed but state not yet loaded. Call status() again."


def test_status_skips_expensive_stats_while_background_index_finalizes():
    mcp = server_module.create_server()
    state = get_state()
    state.codebase_path = Path("/repo")

    class _ExplodingVector:
        def count(self):
            raise AssertionError("vector count should not run during background indexing")

    class _ExplodingGraph:
        def get_statistics(self):
            raise AssertionError("graph stats should not run during background indexing")

    state.vector_engine = _ExplodingVector()
    state.graph_engine = _ExplodingGraph()

    with server_module._index_job_lock:
        server_module._index_job = {"status": "indexing"}

    result = asyncio.run(_call_tool(mcp, "status"))

    assert result["indexed"] is True
    assert "vector_chunks" not in result
    assert "graph" not in result
    assert "indexing" not in result


# --- New exception types ---


def test_authentication_error_is_contextro_exception():
    assert issubclass(AuthenticationError, ContextroException)


def test_authorization_error_is_contextro_exception():
    assert issubclass(AuthorizationError, ContextroException)


def test_rate_limit_error_is_contextro_exception():
    assert issubclass(RateLimitError, ContextroException)
