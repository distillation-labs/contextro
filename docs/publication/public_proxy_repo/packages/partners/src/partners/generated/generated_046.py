"""Generated filler module 046 for the partners package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class PartnersGenerated046:
    identifier: str
    enabled: bool = True


def build_partners_payload_046(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated partners data."""
    record = PartnersGenerated046(identifier=f"{seed}-046")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "partners"}
