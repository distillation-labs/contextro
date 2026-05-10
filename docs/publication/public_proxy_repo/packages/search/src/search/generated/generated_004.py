"""Generated filler module 004 for the search package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class SearchGenerated004:
    identifier: str
    enabled: bool = True


def build_search_payload_004(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated search data."""
    record = SearchGenerated004(identifier=f"{seed}-004")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "search"}
