"""Generated filler module 003 for the partners package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class PartnersGenerated003:
    identifier: str
    enabled: bool = True


def build_partners_payload_003(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated partners data."""
    record = PartnersGenerated003(identifier=f"{seed}-003")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "partners"}
