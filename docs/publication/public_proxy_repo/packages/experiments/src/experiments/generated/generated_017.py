"""Generated filler module 017 for the experiments package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class ExperimentsGenerated017:
    identifier: str
    enabled: bool = True


def build_experiments_payload_017(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated experiments data."""
    record = ExperimentsGenerated017(identifier=f"{seed}-017")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "experiments"}
