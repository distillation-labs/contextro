"""Generated filler test 009 for the auth package."""

from __future__ import annotations

from auth.generated.generated_009 import build_auth_payload_009


def test_generated_payload_009() -> None:
    payload = build_auth_payload_009("seed")
    assert payload["identifier"].startswith("seed-")
