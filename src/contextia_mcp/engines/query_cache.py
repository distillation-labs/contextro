"""Semantic query cache for search results.

Caches search results by query embedding similarity. If a new query
is semantically similar (cosine > threshold) to a cached query,
returns the cached result instantly — zero re-embedding or re-searching.

Typical hit rate: 20-40% in interactive sessions where users refine queries.
"""

from __future__ import annotations

import time
from collections import OrderedDict
from typing import Any


class QueryCache:
    """LRU cache with semantic similarity matching for search queries.

    Stores recent query results and matches new queries against cached
    ones using cosine similarity of their embeddings.
    """

    def __init__(self, max_size: int = 64, similarity_threshold: float = 0.95, ttl: float = 300.0):
        """Initialize the query cache.

        Args:
            max_size: Maximum number of cached entries.
            similarity_threshold: Cosine similarity threshold for cache hits (0.0-1.0).
            ttl: Time-to-live in seconds for cache entries.
        """
        self.max_size = max_size
        self.similarity_threshold = similarity_threshold
        self.ttl = ttl
        self._cache: OrderedDict[tuple[str, str], dict] = OrderedDict()
        self.hits = 0
        self.misses = 0

    def _is_expired(self, entry: dict[str, Any], now: float) -> bool:
        return now - entry["timestamp"] >= self.ttl

    def _prune_expired(self, now: float | None = None) -> None:
        now = time.time() if now is None else now
        expired_keys = [
            key for key, entry in self._cache.items() if self._is_expired(entry, now)
        ]
        for key in expired_keys:
            del self._cache[key]

    def get(
        self,
        query: str,
        query_embedding: list[float] | None = None,
        namespace: str = "",
    ) -> Any | None:
        """Look up a cached result for a query.

        First tries exact string match, then semantic similarity if
        an embedding is provided.

        Args:
            query: The search query string.
            query_embedding: Optional embedding vector for semantic matching.
            namespace: Optional cache namespace for isolating search options.

        Returns:
            Cached result if found, None otherwise.
        """
        cache_key = (namespace, query)
        now = time.time()
        self._prune_expired(now)

        # 1. Exact string match (fastest)
        if cache_key in self._cache:
            entry = self._cache[cache_key]
            self._cache.move_to_end(cache_key)
            self.hits += 1
            return entry["result"]

        # 2. Semantic similarity match (if embedding provided)
        if query_embedding is not None:
            best_match = None
            best_sim = 0.0

            for key, entry in self._cache.items():
                if entry.get("namespace", "") != namespace:
                    continue
                if entry.get("embedding") is None:
                    continue
                sim = self._cosine_similarity(query_embedding, entry["embedding"])
                if sim > best_sim:
                    best_sim = sim
                    best_match = key

            if best_match and best_sim >= self.similarity_threshold:
                self._cache.move_to_end(best_match)
                self.hits += 1
                return self._cache[best_match]["result"]

        self.misses += 1
        return None

    def put(
        self,
        query: str,
        result: Any,
        query_embedding: list[float] | None = None,
        namespace: str = "",
    ) -> None:
        """Store a query result in the cache.

        Args:
            query: The search query string.
            result: The search result to cache.
            query_embedding: Optional embedding for semantic matching.
            namespace: Optional cache namespace for isolating search options.
        """
        cache_key = (namespace, query)
        self._prune_expired()
        if cache_key in self._cache:
            self._cache.move_to_end(cache_key)
            self._cache[cache_key] = {
                "result": result,
                "embedding": query_embedding,
                "namespace": namespace,
                "timestamp": time.time(),
            }
        else:
            if len(self._cache) >= self.max_size:
                self._cache.popitem(last=False)
            self._cache[cache_key] = {
                "result": result,
                "embedding": query_embedding,
                "namespace": namespace,
                "timestamp": time.time(),
            }

    def invalidate(self):
        """Clear all cached entries (e.g., after reindex)."""
        self._cache.clear()

    @property
    def size(self) -> int:
        """Current number of cached entries."""
        return len(self._cache)

    @property
    def hit_rate(self) -> float:
        """Cache hit rate as a fraction."""
        total = self.hits + self.misses
        return self.hits / total if total > 0 else 0.0

    @staticmethod
    def _cosine_similarity(a: list[float], b: list[float]) -> float:
        """Compute cosine similarity between two vectors."""
        if len(a) != len(b):
            return 0.0
        dot = sum(x * y for x, y in zip(a, b))
        norm_a = sum(x * x for x in a) ** 0.5
        norm_b = sum(x * x for x in b) ** 0.5
        if norm_a == 0 or norm_b == 0:
            return 0.0
        return dot / (norm_a * norm_b)
