"""Generated filler test 045 for the auth package."""

from __future__ import annotations

from auth.generated.generated_045 import build_auth_payload_045


def test_generated_payload_045() -> None:
    payload = build_auth_payload_045("seed")
    assert payload["identifier"].startswith("seed-")
