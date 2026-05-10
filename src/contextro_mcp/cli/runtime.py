"""Runtime helpers for non-server CLI subcommands."""

from __future__ import annotations

from pathlib import Path

from contextro_mcp.indexing.pipeline import IndexingPipeline
from contextro_mcp.memory.session_tracker import SessionTracker
from contextro_mcp.state import get_state


def _resolve_root(codebase_path: str | None = None) -> Path:
    """Resolve the current codebase root for CLI-driven workflows."""
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
    return root


def _attach_pipeline_state(state, root: Path, pipeline: IndexingPipeline):
    """Attach freshly indexed engines to session state and clear derived caches."""
    state.codebase_path = root
    state.codebase_paths = [root]
    state.vector_engine = pipeline.vector_engine
    state.bm25_engine = pipeline.bm25_engine
    state.graph_engine = pipeline.graph_engine
    state.clear_derived_caches()
    if hasattr(state, "_query_cache") and state._query_cache:
        state._query_cache.invalidate()

    if not hasattr(state, "_session_tracker") or state._session_tracker is None:
        state._session_tracker = SessionTracker()
    if state.vector_engine:
        state._session_tracker.track_index(str(root), state.vector_engine.count())
    state.capture_index_snapshot()
    return state


def ensure_indexed_state(codebase_path: str | None = None):
    """Ensure the current repository is indexed and attached to session state."""
    state = get_state()
    root = _resolve_root(codebase_path)

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
    return _attach_pipeline_state(state, root, pipeline)


def reindex_state(codebase_path: str | None = None, *, full: bool = False):
    """Force a fresh incremental or full reindex and reattach engines to session state."""
    state = get_state()
    root = _resolve_root(codebase_path)
    pipeline = IndexingPipeline(state.settings)
    if full:
        pipeline.index(root)
    else:
        pipeline.incremental_index(root)
    return _attach_pipeline_state(state, root, pipeline)
