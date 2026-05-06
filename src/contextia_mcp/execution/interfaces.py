"""Provider-agnostic interfaces for shared execution helpers."""

from __future__ import annotations

from typing import Any, Protocol


class VectorSearchBackend(Protocol):
    """Vector search backend used by the shared execution layer."""

    _embedding_service: Any

    def search(self, query: str, limit: int = 10, **kwargs: Any) -> list[dict[str, Any]]:
        """Return semantic search results."""


class KeywordSearchBackend(Protocol):
    """Keyword/FTS backend used by the shared execution layer."""

    def search(self, query: str, limit: int = 10, **kwargs: Any) -> list[dict[str, Any]]:
        """Return keyword search results."""


class GraphSearchBackend(Protocol):
    """Graph backend used by the shared execution layer."""

    def find_nodes_by_name(self, name: str, exact: bool = True) -> list[Any]:
        """Return graph nodes matching a name."""

    def get_node_degree(self, node_id: str) -> tuple[int, int]:
        """Return the in/out degree for a node."""
