"""Generated filler module 002 for the auth package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class AuthGenerated002:
    identifier: str
    enabled: bool = True


def build_auth_payload_002(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated auth data."""
    record = AuthGenerated002(identifier=f"{seed}-002")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "auth"}
