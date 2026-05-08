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
import json
import logging
import re
import time
from collections import OrderedDict
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)


class CompactionArchive:
    """Stores pre-compaction session context for searchable recovery.

    Uses in-memory storage with TTL-based expiry. Content is stored as
    text chunks that can be searched via substring matching or retrieved
    by reference ID.
    """

    def __init__(
        self,
        max_entries: int = 20,
        ttl: float = 86400.0,
        *,
        storage_path: str | Path | None = None,
        memory_store: Any | None = None,
        project: str = "compaction_archive",
    ):
        """Initialize the compaction archive.

        Args:
            max_entries: Maximum archived compaction snapshots.
            ttl: Time-to-live in seconds (default 24h).
        """
        self.max_entries = max_entries
        self.ttl = ttl
        self._storage_path = Path(storage_path).expanduser() if storage_path else None
        self._memory_store = memory_store
        self._project = project
        self._store: OrderedDict[str, dict[str, Any]] = OrderedDict()
        self._load()

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
            self._store[ref_id]["metadata"] = metadata or {}
            self._store[ref_id]["timestamp"] = time.time()
            self._store[ref_id]["chars"] = len(content)
            self._store.move_to_end(ref_id)
            self._persist()
            return ref_id

        if len(self._store) >= self.max_entries:
            oldest_ref, _ = self._store.popitem(last=False)
            self._delete_semantic_entry(oldest_ref)

        self._store[ref_id] = {
            "content": content,
            "metadata": metadata or {},
            "timestamp": time.time(),
            "chars": len(content),
        }
        self._persist()
        self._store_semantic_entry(ref_id, content, metadata or {})
        return ref_id

    def search(self, query: str, limit: int = 5) -> list[dict[str, Any]]:
        """Search archived content by semantic recall plus excerpt extraction.

        Args:
            query: Search query (case-insensitive substring match).
            limit: Maximum results to return.

        Returns:
            List of matching excerpts with context.
        """
        self._prune_expired()
        query = query.strip()
        if not query:
            return []

        results: list[dict[str, Any]] = []
        seen: set[str] = set()

        if self._memory_store is not None:
            for memory in self._memory_store.recall(query, limit=limit, project=self._project):
                entry = self._store.get(memory.id)
                if entry is None:
                    continue
                results.append(self._format_result(memory.id, entry, query, match_type="semantic"))
                seen.add(memory.id)
                if len(results) >= limit:
                    return results

        query_lower = query.lower()
        for ref_id, entry in reversed(self._store.items()):
            if ref_id in seen:
                continue
            if query_lower not in entry["content"].lower():
                continue

            results.append(self._format_result(ref_id, entry, query, match_type="substring"))
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
        self._persist()
        return entry["content"]

    @property
    def size(self) -> int:
        self._prune_expired()
        return len(self._store)

    def _prune_expired(self) -> None:
        now = time.time()
        expired = [k for k, v in self._store.items() if now - v["timestamp"] >= self.ttl]
        changed = False
        for k in expired:
            del self._store[k]
            self._delete_semantic_entry(k)
            changed = True
        if changed:
            self._persist()

    @staticmethod
    def _generate_id(content: str) -> str:
        h = hashlib.md5(content.encode()).hexdigest()[:8]
        return f"ca_{h}"

    def _format_result(
        self,
        ref_id: str,
        entry: dict[str, Any],
        query: str,
        *,
        match_type: str,
    ) -> dict[str, Any]:
        excerpts = self._extract_excerpts(entry["content"], query)
        if not excerpts:
            excerpts = self._fallback_excerpts(entry["content"])

        result = {
            "archive_ref": ref_id,
            "age_minutes": round((time.time() - entry["timestamp"]) / 60, 1),
            "excerpts": excerpts,
        }
        if entry.get("metadata"):
            result["metadata"] = entry["metadata"]
        if match_type:
            result["match"] = match_type
        return result

    @staticmethod
    def _extract_excerpts(content: str, query: str) -> list[str]:
        query_lower = query.lower()
        terms = [term.lower() for term in re.findall(r"[A-Za-z0-9_]+", query) if len(term) >= 3]
        lines = content.split("\n")
        matches: list[str] = []

        for index, line in enumerate(lines):
            line_lower = line.lower()
            if query_lower not in line_lower and not any(term in line_lower for term in terms):
                continue

            start = max(0, index - 2)
            end = min(len(lines), index + 3)
            excerpt = "\n".join(lines[start:end]).strip()
            if excerpt and excerpt not in matches:
                matches.append(excerpt)
            if len(matches) >= 3:
                break

        return matches

    @staticmethod
    def _fallback_excerpts(content: str) -> list[str]:
        lines = [line for line in content.split("\n") if line.strip()]
        if not lines:
            return []
        excerpt = "\n".join(lines[:5])[:500]
        return [excerpt]

    def _load(self) -> None:
        if self._storage_path is None or not self._storage_path.exists():
            return

        try:
            raw_entries = json.loads(self._storage_path.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError) as exc:
            logger.warning("Failed to load compaction archive from %s: %s", self._storage_path, exc)
            return

        now = time.time()
        changed = False
        for raw in raw_entries[-self.max_entries :]:
            ref_id = raw.get("ref_id", "")
            timestamp = float(raw.get("timestamp", 0.0))
            if not ref_id or now - timestamp >= self.ttl:
                if ref_id:
                    self._delete_semantic_entry(ref_id)
                changed = True
                continue

            self._store[ref_id] = {
                "content": raw.get("content", ""),
                "metadata": raw.get("metadata", {}),
                "timestamp": timestamp,
                "chars": int(raw.get("chars", len(raw.get("content", "")))),
            }

        if changed:
            self._persist()

    def _persist(self) -> None:
        if self._storage_path is None:
            return

        self._storage_path.parent.mkdir(parents=True, exist_ok=True)
        payload = [
            {
                "ref_id": ref_id,
                "content": entry["content"],
                "metadata": entry.get("metadata", {}),
                "timestamp": entry["timestamp"],
                "chars": entry["chars"],
            }
            for ref_id, entry in self._store.items()
        ]
        temp_path = self._storage_path.with_suffix(self._storage_path.suffix + ".tmp")
        temp_path.write_text(json.dumps(payload), encoding="utf-8")
        temp_path.replace(self._storage_path)

    def _store_semantic_entry(self, ref_id: str, content: str, metadata: dict[str, Any]) -> None:
        if self._memory_store is None:
            return

        from contextro_mcp.core.models import Memory, MemoryType

        self._memory_store.forget(memory_id=ref_id)
        self._memory_store.remember(
            Memory(
                id=ref_id,
                content=content,
                memory_type=MemoryType.NOTE,
                project=self._project,
                tags=[ref_id],
                ttl=self._ttl_label(),
                source="compaction_archive",
                metadata=metadata,
            )
        )

    def _delete_semantic_entry(self, ref_id: str) -> None:
        if self._memory_store is None:
            return
        self._memory_store.forget(memory_id=ref_id)

    def _ttl_label(self) -> str:
        if self.ttl <= 86400:
            return "day"
        if self.ttl <= 86400 * 7:
            return "week"
        if self.ttl <= 86400 * 31:
            return "month"
        return "permanent"
