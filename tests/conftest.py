"""Shared test fixtures and helpers for Contextia tests."""

import json
import sys
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

SRC_DIR = Path(__file__).resolve().parents[1] / "src"
if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

import contextia_mcp.server as server_module  # noqa: E402
from contextia_mcp.state import reset_state  # noqa: E402


@pytest.fixture(autouse=True)
def clean_state():
    """Reset global state before each test."""
    import os
    reset_state()
    server_module._pipeline = None
    server_module._index_job = {}
    yield
    reset_state()
    server_module._pipeline = None
    server_module._index_job = {}
    # Clean up CTX_STORAGE_DIR set by _setup_indexed
    os.environ.pop("CTX_STORAGE_DIR", None)
    from contextia_mcp.config import reset_settings
    reset_settings()


@pytest.fixture
def mini_codebase(tmp_path):
    """Create a small Python codebase."""
    src = tmp_path / "src"
    src.mkdir()
    (src / "main.py").write_text(
        'def hello():\n    """Say hello."""\n    print("hello")\n'
    )
    (src / "utils.py").write_text(
        'def helper():\n    """A helper."""\n    return 42\n'
    )
    return tmp_path


@pytest.fixture
def mini_codebase_with_calls(tmp_path):
    """Create a Python codebase with call relationships for graph tests."""
    src = tmp_path / "src"
    src.mkdir()
    (src / "main.py").write_text(
        'from utils import helper\n\n\ndef hello():\n    """Say hello."""\n    helper()\n'
    )
    (src / "utils.py").write_text(
        'def helper():\n    """A helper."""\n    return 42\n'
    )
    (src / "app.py").write_text(
        "from main import hello\nfrom utils import helper\n\n\n"
        "def orchestrate():\n"
        '    """Orchestrate calls."""\n'
        "    hello()\n"
        "    helper()\n"
    )
    return tmp_path


def _mock_embedding_service():
    from contextia_mcp.config import get_settings
    from contextia_mcp.indexing.embedding_service import EMBEDDING_MODELS
    settings = get_settings()
    dims = EMBEDDING_MODELS.get(settings.embedding_model, {}).get("dimensions", 384)
    svc = MagicMock()
    svc.embed.return_value = [0.1] * dims
    def dynamic_batch(texts, **kwargs):
        return [[0.1] * dims for _ in texts]
    svc.embed_batch.side_effect = dynamic_batch
    return svc


async def _call_tool(mcp, name, args=None):
    """Call an MCP tool and return the structured result."""
    result = await mcp.call_tool(name, args or {})
    if result.structured_content is not None:
        return result.structured_content.get("result", result.structured_content)
    # Fallback: parse JSON from text content
    for content in result.content:
        if hasattr(content, "text"):
            data = json.loads(content.text)
            return data.get("result", data) if isinstance(data, dict) else data
    return {}


async def _setup_indexed(codebase_path, storage_dir):
    """Index a codebase and return (mcp, mock_svc, result).

    Sets CTX_STORAGE_DIR in os.environ permanently for the duration of the test.
    The clean_state fixture resets settings between tests.
    Handles the background-indexing case by polling status() until indexed: true.
    """
    import asyncio
    import os

    # Set storage dir permanently so subsequent index calls in the same test use it
    os.environ["CTX_STORAGE_DIR"] = str(storage_dir)

    from contextia_mcp.config import reset_settings
    reset_settings()

    with patch("contextia_mcp.indexing.pipeline.get_embedding_service") as mock_get:
        mock_svc = _mock_embedding_service()
        mock_get.return_value = mock_svc

        mcp = server_module.create_server()
        result = await _call_tool(mcp, "index", {"path": str(codebase_path)})

        # If indexing is running in background, poll status() until done
        if result.get("status") == "indexing":
            for _ in range(120):  # up to 60s (120 × 0.5s)
                await asyncio.sleep(0.5)
                status = await _call_tool(mcp, "status")
                with server_module._index_job_lock:
                    job_status = server_module._index_job.get("status")
                    job_result = server_module._index_job.get("result", {})
                    job_error = server_module._index_job.get("error")

                if job_status == "done" and job_result:
                    result = job_result
                    break
                if status.get("index_error") or job_error:
                    raise RuntimeError(
                        f"Background indexing failed: {status.get('index_error') or job_error}"
                    )
            else:
                raise TimeoutError("Background indexing did not complete within 60s")

            # Use the completed job result for assertions
            if not result.get("total_files"):
                with server_module._index_job_lock:
                    job_result = server_module._index_job.get("result", {})
                if job_result:
                    result = job_result

        # Patch vector engine's embedding service for search
        from contextia_mcp.state import get_state
        state = get_state()
        if state.vector_engine:
            state.vector_engine._embedding_service = mock_svc

        return mcp, mock_svc, result
