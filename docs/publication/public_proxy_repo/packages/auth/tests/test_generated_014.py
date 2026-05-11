"""Generated filler test 014 for the auth package."""

from __future__ import annotations

from auth.generated.generated_014 import build_auth_payload_014


def test_generated_payload_014() -> None:
    payload = build_auth_payload_014("seed")
    assert payload["identifier"].startswith("seed-")
