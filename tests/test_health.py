"""Tests for health endpoint and new exceptions (Phase 5f)."""

import asyncio

import pytest

import contextia_mcp.server as server_module
from contextia_mcp.config import reset_settings
from contextia_mcp.core.exceptions import (
    AuthenticationError,
    AuthorizationError,
    ContextiaException,
    RateLimitError,
)
from contextia_mcp.state import reset_state
from tests.conftest import _call_tool


@pytest.fixture(autouse=True)
def clean_state(tmp_path, monkeypatch):
    monkeypatch.setenv("CTX_STORAGE_DIR", str(tmp_path / ".contextia"))
    reset_settings()
    reset_state()
    yield
    reset_settings()
    reset_state()


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


# --- New exception types ---


def test_authentication_error_is_contextia_exception():
    assert issubclass(AuthenticationError, ContextiaException)


def test_authorization_error_is_contextia_exception():
    assert issubclass(AuthorizationError, ContextiaException)


def test_rate_limit_error_is_contextia_exception():
    assert issubclass(RateLimitError, ContextiaException)
