"""Generated filler module 014 for the search package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class SearchGenerated014:
    identifier: str
    enabled: bool = True


def build_search_payload_014(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated search data."""
    record = SearchGenerated014(identifier=f"{seed}-014")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "search"}
