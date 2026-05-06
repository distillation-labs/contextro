"""Session event tracker for context continuity.

Tracks tool calls, search queries, and key decisions during a session.
When context compaction occurs, provides a compressed snapshot of
what happened so the agent doesn't lose track of its work.

Inspired by Context Mode's session continuity approach.
"""

import time
from collections import deque
from dataclasses import dataclass
from typing import Any, Dict


@dataclass
class SessionEvent:
    """A single session event."""
    timestamp: float
    event_type: str  # "search", "explain", "index", "remember", "find_symbol"
    summary: str  # One-line summary
    priority: int = 1  # 1=low, 2=medium, 3=high


class SessionTracker:
    """Tracks session events for context continuity.

    Maintains a rolling window of recent events and can produce
    a priority-tiered snapshot for context recovery.
    """

    def __init__(self, max_events: int = 100):
        """Initialize the session tracker.

        Args:
            max_events: Maximum events to retain.
        """
        self._events: deque = deque(maxlen=max_events)
        self._search_queries: deque = deque(maxlen=20)
        self._symbols_explored: deque = deque(maxlen=20)
        self._files_touched: set = set()
        self.started_at = time.time()

    def track_search(self, query: str, result_count: int):
        """Track a search query."""
        self._search_queries.append(query)
        self._events.append(SessionEvent(
            timestamp=time.time(),
            event_type="search",
            summary=f"search({query!r}) → {result_count} results",
            priority=1,
        ))

    def track_explain(self, symbol_name: str):
        """Track an explain call."""
        self._symbols_explored.append(symbol_name)
        self._events.append(SessionEvent(
            timestamp=time.time(),
            event_type="explain",
            summary=f"explain({symbol_name!r})",
            priority=2,
        ))

    def track_find_symbol(self, symbol_name: str, found: bool):
        """Track a find_symbol call."""
        self._symbols_explored.append(symbol_name)
        status = "found" if found else "not found"
        self._events.append(SessionEvent(
            timestamp=time.time(),
            event_type="find_symbol",
            summary=f"find_symbol({symbol_name!r}) → {status}",
            priority=2,
        ))

    def track_impact(self, symbol_name: str, impacted_count: int):
        """Track an impact analysis."""
        self._events.append(SessionEvent(
            timestamp=time.time(),
            event_type="impact",
            summary=f"impact({symbol_name!r}) → {impacted_count} affected",
            priority=3,
        ))

    def track_index(self, path: str, chunks: int):
        """Track an indexing operation."""
        self._events.append(SessionEvent(
            timestamp=time.time(),
            event_type="index",
            summary=f"indexed {path} ({chunks} chunks)",
            priority=3,
        ))

    def track_remember(self, content_preview: str):
        """Track a memory store."""
        self._events.append(SessionEvent(
            timestamp=time.time(),
            event_type="remember",
            summary=f"stored: {content_preview[:60]}",
            priority=2,
        ))

    def get_snapshot(self, max_tokens: int = 500) -> Dict[str, Any]:
        """Generate a compressed session snapshot for context recovery.

        Returns a priority-tiered summary under the token budget.

        Args:
            max_tokens: Maximum tokens for the snapshot (~4 chars/token).

        Returns:
            Compact session state dict.
        """
        max_chars = max_tokens * 4
        snapshot: Dict[str, Any] = {
            "session_duration_min": round((time.time() - self.started_at) / 60, 1),
            "total_events": len(self._events),
        }

        # High-priority events first
        high = [e for e in self._events if e.priority >= 3]
        if high:
            snapshot["key_actions"] = [e.summary for e in high[-5:]]

        # Recent searches
        if self._search_queries:
            snapshot["recent_searches"] = list(self._search_queries)[-5:]

        # Symbols explored
        if self._symbols_explored:
            snapshot["symbols_explored"] = list(set(self._symbols_explored))[:10]

        # Check budget
        import json
        text = json.dumps(snapshot)
        if len(text) > max_chars:
            # Trim to fit
            snapshot.pop("recent_searches", None)
            if len(json.dumps(snapshot)) > max_chars:
                snapshot.pop("symbols_explored", None)

        return snapshot

    @property
    def event_count(self) -> int:
        """Total events tracked."""
        return len(self._events)
