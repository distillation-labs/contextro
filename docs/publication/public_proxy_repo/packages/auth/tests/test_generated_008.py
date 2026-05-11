"""Generated filler test 008 for the auth package."""

from __future__ import annotations

from auth.generated.generated_008 import build_auth_payload_008


def test_generated_payload_008() -> None:
    payload = build_auth_payload_008("seed")
    assert payload["identifier"].startswith("seed-")
