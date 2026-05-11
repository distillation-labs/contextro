"""Generated filler module 040 for the partners package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class PartnersGenerated040:
    identifier: str
    enabled: bool = True


def build_partners_payload_040(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated partners data."""
    record = PartnersGenerated040(identifier=f"{seed}-040")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "partners"}
