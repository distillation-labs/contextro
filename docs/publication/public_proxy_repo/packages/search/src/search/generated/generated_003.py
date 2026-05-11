"""Generated filler module 003 for the search package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class SearchGenerated003:
    identifier: str
    enabled: bool = True


def build_search_payload_003(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated search data."""
    record = SearchGenerated003(identifier=f"{seed}-003")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "search"}
