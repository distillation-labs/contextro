"""Generated filler module 017 for the search package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class SearchGenerated017:
    identifier: str
    enabled: bool = True


def build_search_payload_017(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated search data."""
    record = SearchGenerated017(identifier=f"{seed}-017")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "search"}
