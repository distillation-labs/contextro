"""Generated filler module 004 for the experiments package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class ExperimentsGenerated004:
    identifier: str
    enabled: bool = True


def build_experiments_payload_004(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated experiments data."""
    record = ExperimentsGenerated004(identifier=f"{seed}-004")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "experiments"}
