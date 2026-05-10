"""Generated filler test 018 for the auth package."""

from __future__ import annotations

from auth.generated.generated_018 import build_auth_payload_018


def test_generated_payload_018() -> None:
    payload = build_auth_payload_018("seed")
    assert payload["identifier"].startswith("seed-")
