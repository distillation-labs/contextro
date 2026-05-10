"""Generated filler module 040 for the search package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class SearchGenerated040:
    identifier: str
    enabled: bool = True


def build_search_payload_040(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated search data."""
    record = SearchGenerated040(identifier=f"{seed}-040")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "search"}
