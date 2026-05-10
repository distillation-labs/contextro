"""Generated filler module 013 for the search package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class SearchGenerated013:
    identifier: str
    enabled: bool = True


def build_search_payload_013(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated search data."""
    record = SearchGenerated013(identifier=f"{seed}-013")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "search"}
