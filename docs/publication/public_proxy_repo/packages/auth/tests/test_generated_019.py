"""Generated filler test 019 for the auth package."""

from __future__ import annotations

from auth.generated.generated_019 import build_auth_payload_019


def test_generated_payload_019() -> None:
    payload = build_auth_payload_019("seed")
    assert payload["identifier"].startswith("seed-")
