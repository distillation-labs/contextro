"""Shared helpers for Contextro benchmark scripts."""

from __future__ import annotations

import asyncio
import gc
import json
import os
from contextlib import contextmanager
from pathlib import Path
from unittest.mock import MagicMock, patch

CHARS_PER_TOKEN = 4


def estimate_tokens(obj: object) -> int:
    """Estimate token count for a JSON-serializable object."""
    if isinstance(obj, str):
        return len(obj) // CHARS_PER_TOKEN
    return len(json.dumps(obj, default=str)) // CHARS_PER_TOKEN


def estimate_tokens_toon(obj: object) -> int:
    """Estimate token count using TOON encoding."""
    from contextro_mcp.formatting.toon_encoder import toon_encode

    return len(toon_encode(obj)) // CHARS_PER_TOKEN


def create_mock_embedding_service(dims: int = 384) -> MagicMock:
    """Create a deterministic mock embedding service for benchmarks."""
    svc = MagicMock()
    svc.embed.return_value = [0.1] * dims
    svc.embed_batch.side_effect = lambda texts, **kwargs: [[0.1] * dims for _ in texts]
    return svc


async def call_tool(mcp, name: str, args: dict | None = None) -> dict:
    """Call an MCP tool and normalize its JSON response."""
    result = await mcp.call_tool(name, args or {})
    if result.structured_content is not None:
        return result.structured_content.get("result", result.structured_content)

    for content in result.content:
        if hasattr(content, "text"):
            data = json.loads(content.text)
            return data.get("result", data) if isinstance(data, dict) else data

    return {}


async def index_codebase(mcp, server_module, path: str, timeout_seconds: int = 180) -> dict:
    """Index a codebase and wait for the final result when indexing runs in the background."""
    result = await call_tool(mcp, "index", {"path": path})
    if result.get("status") != "indexing":
        return result

    for _ in range(timeout_seconds * 2):
        await asyncio.sleep(0.5)
        status = await call_tool(mcp, "status")
        with server_module._index_job_lock:
            job_status = server_module._index_job.get("status")
            job_result = server_module._index_job.get("result", {})
            job_error = server_module._index_job.get("error")

        if job_status == "done" and job_result:
            return job_result
        if status.get("index_error") or job_error:
            raise RuntimeError(status.get("index_error") or job_error)

    raise TimeoutError(f"Indexing did not complete within {timeout_seconds}s")


def sync_vector_engine(state, mock_embedding_service) -> None:
    """Patch the in-memory vector engine to use the benchmark embedding mock."""
    if state.vector_engine:
        state.vector_engine._embedding_service = mock_embedding_service


@contextmanager
def benchmark_session(
    storage_dir: Path,
    *,
    embedding_model: str = "bge-small-en",
    dims: int = 384,
    env_overrides: dict[str, str] | None = None,
):
    """Create a patched MCP session suitable for lightweight benchmarks."""

    import contextro_mcp.indexing.pipeline as pipeline_module
    import contextro_mcp.server as server_module
    from contextro_mcp.config import reset_settings
    from contextro_mcp.state import get_state, reset_state

    storage_dir.mkdir(parents=True, exist_ok=True)
    mock_svc = create_mock_embedding_service(dims)
    env = {
        "CTX_STORAGE_DIR": str(storage_dir),
        "CTX_EMBEDDING_MODEL": embedding_model,
    }
    if env_overrides:
        env.update(env_overrides)

    with (
        patch.object(pipeline_module, "get_embedding_service", return_value=mock_svc),
        patch.dict(
            os.environ,
            env,
            clear=False,
        ),
    ):
        reset_settings()
        reset_state()
        server_module._pipeline = None
        server_module._index_job = {}

        try:
            yield server_module.create_server(), mock_svc, server_module
        finally:
            try:
                get_state().shutdown()
            except Exception:
                pass
            reset_settings()
            reset_state()
            server_module._pipeline = None
            server_module._index_job = {}
            gc.collect()
