"""Generated filler test 015 for the auth package."""

from __future__ import annotations

from auth.generated.generated_015 import build_auth_payload_015


def test_generated_payload_015() -> None:
    payload = build_auth_payload_015("seed")
    assert payload["identifier"].startswith("seed-")
