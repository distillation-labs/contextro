"""Generated filler module 001 for the partners package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class PartnersGenerated001:
    identifier: str
    enabled: bool = True


def build_partners_payload_001(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated partners data."""
    record = PartnersGenerated001(identifier=f"{seed}-001")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "partners"}
