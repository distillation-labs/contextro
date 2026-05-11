"""Generated filler module 002 for the shared package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class SharedGenerated002:
    identifier: str
    enabled: bool = True


def build_shared_payload_002(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated shared data."""
    record = SharedGenerated002(identifier=f"{seed}-002")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "shared"}
