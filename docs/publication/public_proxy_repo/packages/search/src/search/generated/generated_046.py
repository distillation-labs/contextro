"""Generated filler module 046 for the search package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class SearchGenerated046:
    identifier: str
    enabled: bool = True


def build_search_payload_046(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated search data."""
    record = SearchGenerated046(identifier=f"{seed}-046")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "search"}
