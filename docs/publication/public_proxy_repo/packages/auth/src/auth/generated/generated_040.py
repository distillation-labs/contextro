"""Generated filler module 040 for the auth package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class AuthGenerated040:
    identifier: str
    enabled: bool = True


def build_auth_payload_040(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated auth data."""
    record = AuthGenerated040(identifier=f"{seed}-040")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "auth"}
