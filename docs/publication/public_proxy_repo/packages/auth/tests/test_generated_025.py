"""Generated filler test 025 for the auth package."""

from __future__ import annotations

from auth.generated.generated_025 import build_auth_payload_025


def test_generated_payload_025() -> None:
    payload = build_auth_payload_025("seed")
    assert payload["identifier"].startswith("seed-")
