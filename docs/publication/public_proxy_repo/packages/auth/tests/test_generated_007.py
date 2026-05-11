"""Generated filler test 007 for the auth package."""

from __future__ import annotations

from auth.generated.generated_007 import build_auth_payload_007


def test_generated_payload_007() -> None:
    payload = build_auth_payload_007("seed")
    assert payload["identifier"].startswith("seed-")
