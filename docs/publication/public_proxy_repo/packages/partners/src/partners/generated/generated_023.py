"""Generated filler module 023 for the partners package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class PartnersGenerated023:
    identifier: str
    enabled: bool = True


def build_partners_payload_023(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated partners data."""
    record = PartnersGenerated023(identifier=f"{seed}-023")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "partners"}
