"""Searchable compaction archive for session context recovery.

When context compaction occurs, the full pre-compaction history is stored
here so the agent can search it to recover specific details that were lost
in summarization.

Inspired by:
- Cursor's "chat history as files" for summarization recovery
- Anthropic's principle that compaction is lossy and agents need recovery
- Devin's finding that custom compaction outperforms model self-summarization
"""

from __future__ import annotations

import hashlib
import time
from collections import OrderedDict
from typing import Any


class CompactionArchive:
    """Stores pre-compaction session context for searchable recovery.

    Uses in-memory storage with TTL-based expiry. Content is stored as
    text chunks that can be searched via substring matching or retrieved
    by reference ID.
    """

    def __init__(self, max_entries: int = 20, ttl: float = 86400.0):
        """Initialize the compaction archive.

        Args:
            max_entries: Maximum archived compaction snapshots.
            ttl: Time-to-live in seconds (default 24h).
        """
        self.max_entries = max_entries
        self.ttl = ttl
        self._store: OrderedDict[str, dict[str, Any]] = OrderedDict()

    def archive(self, content: str, metadata: dict[str, Any] | None = None) -> str:
        """Archive pre-compaction content.

        Args:
            content: The full pre-compaction context (e.g., message history).
            metadata: Optional metadata (session_id, event_count, etc.).

        Returns:
            A reference ID for retrieval.
        """
        self._prune_expired()

        ref_id = self._generate_id(content)
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
        return ref_id

    def search(self, query: str, limit: int = 5) -> list[dict[str, Any]]:
        """Search archived content by substring matching.

        Args:
            query: Search query (case-insensitive substring match).
            limit: Maximum results to return.

        Returns:
            List of matching excerpts with context.
        """
        self._prune_expired()
        query_lower = query.lower()
        results: list[dict[str, Any]] = []

        for ref_id, entry in reversed(self._store.items()):
            content = entry["content"]
            lines = content.split("\n")
            matches: list[str] = []

            for i, line in enumerate(lines):
                if query_lower in line.lower():
                    # Include surrounding context (2 lines before/after)
                    start = max(0, i - 2)
                    end = min(len(lines), i + 3)
                    excerpt = "\n".join(lines[start:end])
                    if excerpt not in matches:
                        matches.append(excerpt)
                    if len(matches) >= 3:
                        break

            if matches:
                results.append({
                    "archive_ref": ref_id,
                    "age_minutes": round((time.time() - entry["timestamp"]) / 60, 1),
                    "excerpts": matches,
                })
                if len(results) >= limit:
                    break

        return results

    def retrieve(self, ref_id: str) -> str | None:
        """Retrieve full archived content by reference ID."""
        self._prune_expired()
        entry = self._store.get(ref_id)
        if entry is None:
            return None
        self._store.move_to_end(ref_id)
        return entry["content"]

    @property
    def size(self) -> int:
        return len(self._store)

    def _prune_expired(self) -> None:
        now = time.time()
        expired = [k for k, v in self._store.items() if now - v["timestamp"] >= self.ttl]
        for k in expired:
            del self._store[k]

    @staticmethod
    def _generate_id(content: str) -> str:
        h = hashlib.md5(content.encode()).hexdigest()[:8]
        return f"ca_{h}"
