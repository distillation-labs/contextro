"""Shared runtime builders for Contextia execution helpers."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any

from contextia_mcp.config import Settings
from contextia_mcp.engines.output_sandbox import OutputSandbox
from contextia_mcp.engines.query_cache import QueryCache
from contextia_mcp.execution.interfaces import (
    GraphSearchBackend,
    KeywordSearchBackend,
    VectorSearchBackend,
)
from contextia_mcp.memory.session_tracker import SessionTracker


@dataclass(slots=True)
class SearchRuntime:
    """Concrete runtime dependencies for the shared search execution layer."""

    state: Any
    settings: Settings
    codebase_path: Path | None
    codebase_paths: tuple[Path, ...]
    vector_engine: VectorSearchBackend | None
    bm25_engine: KeywordSearchBackend | None
    graph_engine: GraphSearchBackend | None
    query_cache: QueryCache
    output_sandbox: OutputSandbox
    session_tracker: SessionTracker


def _coerce_codebase_paths(state: Any) -> tuple[Path, ...]:
    raw_paths = getattr(state, "codebase_paths", None) or []
    if not raw_paths and getattr(state, "codebase_path", None):
        raw_paths = [state.codebase_path]
    return tuple(Path(path) for path in raw_paths if path)


def build_search_runtime(state: Any, settings: Settings) -> SearchRuntime:
    """Build the shared runtime used by search-related tool flows."""

    if not hasattr(state, "_query_cache") or state._query_cache is None:
        state._query_cache = QueryCache(
            max_size=settings.search_cache_max_size,
            similarity_threshold=settings.search_cache_similarity_threshold,
            ttl=settings.search_cache_ttl_seconds,
        )

    if not hasattr(state, "_session_tracker") or state._session_tracker is None:
        state._session_tracker = SessionTracker()

    if not hasattr(state, "_output_sandbox") or state._output_sandbox is None:
        state._output_sandbox = OutputSandbox(
            max_entries=settings.search_sandbox_max_entries,
            ttl=settings.search_sandbox_ttl_seconds,
        )

    return SearchRuntime(
        state=state,
        settings=settings,
        codebase_path=getattr(state, "codebase_path", None),
        codebase_paths=_coerce_codebase_paths(state),
        vector_engine=getattr(state, "vector_engine", None),
        bm25_engine=getattr(state, "bm25_engine", None),
        graph_engine=getattr(state, "graph_engine", None),
        query_cache=state._query_cache,
        output_sandbox=state._output_sandbox,
        session_tracker=state._session_tracker,
    )
