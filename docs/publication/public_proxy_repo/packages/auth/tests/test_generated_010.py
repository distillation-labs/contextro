"""Generated filler test 010 for the auth package."""

from __future__ import annotations

from auth.generated.generated_010 import build_auth_payload_010


def test_generated_payload_010() -> None:
    payload = build_auth_payload_010("seed")
    assert payload["identifier"].startswith("seed-")
