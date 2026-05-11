"""Generated filler module 012 for the search package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class SearchGenerated012:
    identifier: str
    enabled: bool = True


def build_search_payload_012(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated search data."""
    record = SearchGenerated012(identifier=f"{seed}-012")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "search"}
