"""Generated filler module 017 for the partners package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class PartnersGenerated017:
    identifier: str
    enabled: bool = True


def build_partners_payload_017(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated partners data."""
    record = PartnersGenerated017(identifier=f"{seed}-017")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "partners"}
