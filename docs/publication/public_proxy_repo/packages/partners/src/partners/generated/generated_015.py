"""Generated filler module 015 for the partners package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class PartnersGenerated015:
    identifier: str
    enabled: bool = True


def build_partners_payload_015(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated partners data."""
    record = PartnersGenerated015(identifier=f"{seed}-015")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "partners"}
