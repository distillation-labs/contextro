"""Runtime helpers for non-server CLI subcommands."""

from __future__ import annotations

from pathlib import Path

from contextro_mcp.indexing.pipeline import IndexingPipeline
from contextro_mcp.memory.session_tracker import SessionTracker
from contextro_mcp.state import get_state


def ensure_indexed_state(codebase_path: str | None = None):
    """Ensure the current repository is indexed and attached to session state."""
    state = get_state()
    root = (
        Path(codebase_path).expanduser().resolve()
        if codebase_path
        else Path(state.codebase_path).resolve()
        if state.codebase_path
        else Path.cwd().resolve()
    )
    if not root.is_dir():
        raise ValueError(f"Codebase path is not a directory: {root}")

    if (
        state.is_indexed
        and state.codebase_path
        and Path(state.codebase_path).resolve() == root
        and state.graph_engine is not None
        and state.vector_engine is not None
    ):
        return state

    pipeline = IndexingPipeline(state.settings)
    pipeline.incremental_index(root)

    state.codebase_path = root
    state.codebase_paths = [root]
    state.vector_engine = pipeline.vector_engine
    state.bm25_engine = pipeline.bm25_engine
    state.graph_engine = pipeline.graph_engine
    state._repository_map_cache = None
    state._static_analysis_cache = None

    if not hasattr(state, "_session_tracker") or state._session_tracker is None:
        state._session_tracker = SessionTracker()
    if state.vector_engine:
        state._session_tracker.track_index(str(root), state.vector_engine.count())
    state.capture_index_snapshot()
    return state
