"""Session state management for Contextro server."""

import logging
import threading
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Optional

from contextro_mcp.config import Settings, get_settings

logger = logging.getLogger(__name__)


@dataclass
class SessionState:
    """Singleton state for the MCP server session."""

    settings: Settings = field(default_factory=get_settings)
    codebase_path: Optional[Path] = None
    codebase_paths: list = field(default_factory=list)
    started_at: float = field(default_factory=time.time)

    # Engine references (populated during indexing)
    _vector_engine: Any = None
    _graph_engine: Any = None
    _bm25_engine: Any = None
    _memory_store: Any = None
    _shutting_down: bool = False
    _shutdown_lock: threading.Lock = field(default_factory=threading.Lock)

    # Git integration (populated on first index)
    _commit_indexer: Any = None
    _branch_watcher: Any = None
    _cross_repo_manager: Any = None
    _file_watcher: Any = None
    current_branch: Optional[str] = None
    current_head: Optional[str] = None

    @property
    def is_indexed(self) -> bool:
        """Check if a codebase has been indexed.

        On first call, attempts to warm-start from persisted data
        (LanceDB on disk + SQLite graph) so agents don't need to
        explicitly call 'index' after server restart.
        """
        if not self.settings.auto_warm_start:
            return self.codebase_path is not None
        if self.codebase_path is not None:
            return True
        # Try auto-warm-start from persisted metadata
        if not hasattr(self, "_warm_start_attempted"):
            self._warm_start_attempted = True
            self._try_warm_start()
        return self.codebase_path is not None

    def _try_warm_start(self) -> None:
        """Attempt to restore index state from persisted data on disk."""
        import json

        settings = self.settings
        metadata_path = settings.storage_path / "index_metadata.json"

        if not metadata_path.exists():
            return

        try:
            data = json.loads(metadata_path.read_text())
            codebase_path = data.get("codebase_path")
            if not codebase_path:
                return

            codebase_path = Path(codebase_path)
            if not codebase_path.is_dir():
                return

            # Restore vector engine (LanceDB is disk-backed, just reconnect)
            from contextro_mcp.engines.bm25_engine import LanceDBBM25Engine
            from contextro_mcp.engines.vector_engine import LanceDBVectorEngine
            from contextro_mcp.indexing.embedding_service import (
                EMBEDDING_MODELS,
                get_embedding_service,
            )

            model_config = EMBEDDING_MODELS.get(settings.embedding_model, {})
            embedding_service = get_embedding_service(settings.embedding_model)

            vector_engine = LanceDBVectorEngine(
                db_path=str(settings.lancedb_path),
                embedding_service=embedding_service,
                vector_dims=model_config.get("dimensions", 256),
            )

            # Check if vector table has data
            if vector_engine.count() == 0:
                return

            self._vector_engine = vector_engine

            # Restore BM25 engine
            bm25_engine = LanceDBBM25Engine(db_path=str(settings.lancedb_path))
            bm25_engine.ensure_fts_index()
            self._bm25_engine = bm25_engine

            # Restore graph from SQLite
            from contextro_mcp.persistence.store import GraphPersistence

            persistence = GraphPersistence(str(settings.graph_path))
            if persistence.exists():
                graph = persistence.load()
                if graph and graph.get_statistics().get("total_nodes", 0) > 0:
                    self._graph_engine = graph

            # If no graph persisted, create empty one
            if self._graph_engine is None:
                from contextro_mcp.engines.graph_engine import RustworkxCodeGraph

                self._graph_engine = RustworkxCodeGraph()

            self.codebase_path = codebase_path
            self.codebase_paths = [codebase_path]

            # Restore multi-path if available
            if "codebase_paths" in data:
                self.codebase_paths = [Path(p) for p in data["codebase_paths"]]

            # Restore git branch info
            try:
                from contextro_mcp.git.commit_indexer import (
                    get_current_branch,
                    get_current_head,
                    is_git_repo,
                )

                if is_git_repo(str(codebase_path)):
                    self.current_branch = get_current_branch(str(codebase_path))
                    self.current_head = get_current_head(str(codebase_path))
            except Exception:
                pass

            # Pre-warm search: run a dummy query so first real search doesn't timeout
            try:
                vector_engine.search("warmup", limit=1)
            except Exception:
                pass  # Non-critical — search will still work, just slower on first call

            logger.info(
                "Auto-warm-start: restored index for %s (%d chunks, %d graph nodes)",
                codebase_path,
                vector_engine.count(),
                self._graph_engine.get_statistics().get("total_nodes", 0)
                if self._graph_engine
                else 0,
            )
        except Exception as e:
            logger.warning("Auto-warm-start failed: %s", e)

    @property
    def vector_engine(self):
        return self._vector_engine

    @vector_engine.setter
    def vector_engine(self, engine):
        self._vector_engine = engine

    @property
    def graph_engine(self):
        return self._graph_engine

    @graph_engine.setter
    def graph_engine(self, engine):
        self._graph_engine = engine

    @property
    def bm25_engine(self):
        return self._bm25_engine

    @bm25_engine.setter
    def bm25_engine(self, engine):
        self._bm25_engine = engine

    @property
    def memory_store(self):
        return self._memory_store

    @memory_store.setter
    def memory_store(self, store):
        self._memory_store = store

    @property
    def commit_indexer(self):
        return self._commit_indexer

    @commit_indexer.setter
    def commit_indexer(self, indexer):
        self._commit_indexer = indexer

    @property
    def branch_watcher(self):
        return self._branch_watcher

    @branch_watcher.setter
    def branch_watcher(self, watcher):
        self._branch_watcher = watcher

    @property
    def cross_repo_manager(self):
        return self._cross_repo_manager

    @cross_repo_manager.setter
    def cross_repo_manager(self, manager):
        self._cross_repo_manager = manager

    @property
    def file_watcher_instance(self):
        return self._file_watcher

    @file_watcher_instance.setter
    def file_watcher_instance(self, watcher):
        self._file_watcher = watcher

    @property
    def shutting_down(self) -> bool:
        return self._shutting_down

    def shutdown(self) -> None:
        """Gracefully shut down: persist graph state and clean up resources."""
        with self._shutdown_lock:
            if self._shutting_down:
                return
            self._shutting_down = True

        # Stop branch watcher
        if self._branch_watcher:
            try:
                self._branch_watcher.stop()
                logger.info("Branch watcher stopped.")
            except Exception as e:
                logger.warning("Failed to stop branch watcher: %s", e)

        # Stop file watcher
        if self._file_watcher:
            try:
                import asyncio

                loop = asyncio.get_event_loop()
                if loop.is_running():
                    loop.create_task(self._file_watcher.stop())
                else:
                    loop.run_until_complete(self._file_watcher.stop())
            except Exception as e:
                logger.warning("Failed to stop file watcher: %s", e)

        # Persist graph to SQLite if populated
        if self._graph_engine and self.codebase_path:
            try:
                from contextro_mcp.persistence.store import GraphPersistence

                persistence = GraphPersistence(str(self.settings.graph_path))
                persistence.save(self._graph_engine)
                logger.info("Graph state persisted to %s", self.settings.graph_path)
            except Exception as e:
                logger.warning("Failed to persist graph state: %s", e)

        logger.info("Contextro shutdown complete.")


_state: Optional[SessionState] = None


def get_state() -> SessionState:
    """Get or create the singleton session state."""
    global _state
    if _state is None:
        _state = SessionState()
    return _state


def reset_state() -> None:
    """Reset session state (for testing)."""
    global _state
    _state = None
