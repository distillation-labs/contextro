"""Generated filler module 015 for the search package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class SearchGenerated015:
    identifier: str
    enabled: bool = True


def build_search_payload_015(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated search data."""
    record = SearchGenerated015(identifier=f"{seed}-015")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "search"}
