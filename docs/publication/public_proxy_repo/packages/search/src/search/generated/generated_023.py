"""Generated filler module 023 for the search package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class SearchGenerated023:
    identifier: str
    enabled: bool = True


def build_search_payload_023(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated search data."""
    record = SearchGenerated023(identifier=f"{seed}-023")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "search"}
