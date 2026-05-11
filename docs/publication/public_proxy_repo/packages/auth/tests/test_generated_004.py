"""Generated filler test 004 for the auth package."""

from __future__ import annotations

from auth.generated.generated_004 import build_auth_payload_004


def test_generated_payload_004() -> None:
    payload = build_auth_payload_004("seed")
    assert payload["identifier"].startswith("seed-")
