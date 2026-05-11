"""Generated filler test 011 for the auth package."""

from __future__ import annotations

from auth.generated.generated_011 import build_auth_payload_011


def test_generated_payload_011() -> None:
    payload = build_auth_payload_011("seed")
    assert payload["identifier"].startswith("seed-")
