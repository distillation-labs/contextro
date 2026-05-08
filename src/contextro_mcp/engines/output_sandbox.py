"""Output sandbox for large tool results.

Stores large outputs externally and returns lightweight references.
The agent can retrieve specific content on demand via the `retrieve` tool.

Inspired by Context Mode's sandboxing pattern which achieves 98% token reduction
on large outputs by indexing them locally and returning only summaries.
"""

from __future__ import annotations

import hashlib
import time
from collections import OrderedDict
from typing import Any


class OutputSandbox:
    """Stores large tool outputs and provides retrieval by reference.

    When a search returns many results with large code snippets,
    the sandbox stores the full snippets and returns only metadata.
    The agent can then retrieve specific snippets on demand.
    """

    def __init__(self, max_entries: int = 100, ttl: float = 600.0):
        """Initialize the output sandbox.

        Args:
            max_entries: Maximum stored entries before LRU eviction.
            ttl: Time-to-live in seconds for stored entries.
        """
        self.max_entries = max_entries
        self.ttl = ttl
        self._store: OrderedDict[str, dict[str, Any]] = OrderedDict()
        self.total_stored_chars = 0
        self.total_retrieved_chars = 0

    def _is_expired(self, entry: dict[str, Any], now: float) -> bool:
        return now - entry["timestamp"] >= self.ttl

    def _prune_expired(self, now: float | None = None) -> None:
        now = time.time() if now is None else now
        expired = [ref_id for ref_id, entry in self._store.items() if self._is_expired(entry, now)]
        for ref_id in expired:
            del self._store[ref_id]

    def store(self, content: str, metadata: dict[str, Any] | None = None) -> str:
        """Store content and return a reference ID.

        Args:
            content: The content to store (e.g., code snippet).
            metadata: Optional metadata about the content.

        Returns:
            A short reference ID for retrieval.
        """
        ref_id = self._generate_id(content)
        self._prune_expired()

        if ref_id in self._store:
            self._store.move_to_end(ref_id)
            return ref_id

        if len(self._store) >= self.max_entries:
            self._store.popitem(last=False)

        self._store[ref_id] = {
            "content": content,
            "metadata": metadata or {},
            "timestamp": time.time(),
            "chars": len(content),
        }
        self.total_stored_chars += len(content)
        return ref_id

    def retrieve(self, ref_id: str, query: str | None = None) -> str | None:
        """Retrieve stored content by reference ID.

        Args:
            ref_id: The reference ID returned by store().
            query: Optional query to filter/highlight relevant sections.

        Returns:
            The stored content, or None if not found/expired.
        """
        self._prune_expired()
        entry = self._store.get(ref_id)
        if entry is None:
            return None

        self._store.move_to_end(ref_id)
        content = entry["content"]
        self.total_retrieved_chars += len(content)

        # If query provided, return only matching lines
        if query:
            query_lower = query.lower()
            lines = content.split("\n")
            matching = [line for line in lines if query_lower in line.lower()]
            if matching:
                return "\n".join(matching[:20])

        return content

    def store_results(self, results: list[dict[str, Any]]) -> str:
        """Store a batch of search results and return a sandbox reference.

        Args:
            results: List of search result dicts with code_snippet fields.

        Returns:
            A reference ID for the batch.
        """
        # Combine all snippets into a single indexed document
        parts = []
        for i, r in enumerate(results):
            snippet = r.get("code_snippet", "")
            if snippet:
                fp = r.get("filepath", "unknown")
                line = r.get("line_start", 0)
                parts.append(f"--- {fp}:{line} ---\n{snippet}")

        combined = "\n\n".join(parts)
        return self.store(combined, metadata={"result_count": len(results)})

    @property
    def size(self) -> int:
        """Number of stored entries."""
        return len(self._store)

    @property
    def savings_chars(self) -> int:
        """Total characters kept out of context."""
        return self.total_stored_chars - self.total_retrieved_chars

    def _generate_id(self, content: str) -> str:
        """Generate a short reference ID from content hash."""
        h = hashlib.md5(content.encode()).hexdigest()[:8]
        return f"sx_{h}"
