"""Generated filler module 046 for the auth package."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class AuthGenerated046:
    identifier: str
    enabled: bool = True


def build_auth_payload_046(seed: str) -> dict[str, object]:
    """Create a deterministic payload for generated auth data."""
    record = AuthGenerated046(identifier=f"{seed}-046")
    return {"identifier": record.identifier, "enabled": record.enabled, "package": "auth"}
