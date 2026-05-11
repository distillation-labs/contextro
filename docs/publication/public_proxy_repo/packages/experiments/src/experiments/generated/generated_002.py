"""Generated filler module 002 for the experiments package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class ExperimentsGenerated002:
    identifier: str
    enabled: bool = True


def build_experiments_payload_002(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated experiments data."""
    record = ExperimentsGenerated002(identifier=f"{seed}-002")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "experiments"}
