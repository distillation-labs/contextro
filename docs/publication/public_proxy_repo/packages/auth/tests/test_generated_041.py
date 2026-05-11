"""Generated filler test 041 for the auth package."""

from __future__ import annotations

from auth.generated.generated_041 import build_auth_payload_041


def test_generated_payload_041() -> None:
    payload = build_auth_payload_041("seed")
    assert payload["identifier"].startswith("seed-")
