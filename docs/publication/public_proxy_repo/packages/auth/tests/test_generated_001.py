"""Generated filler test 001 for the auth package."""

from __future__ import annotations

from auth.generated.generated_001 import build_auth_payload_001


def test_generated_payload_001() -> None:
    payload = build_auth_payload_001("seed")
    assert payload["identifier"].startswith("seed-")
