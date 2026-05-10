"""Generated filler module 023 for the auth package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class AuthGenerated023:
    identifier: str
    enabled: bool = True


def build_auth_payload_023(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated auth data."""
    record = AuthGenerated023(identifier=f"{seed}-023")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "auth"}
