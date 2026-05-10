"""Generated filler test 016 for the auth package."""

from __future__ import annotations

from auth.generated.generated_016 import build_auth_payload_016


def test_generated_payload_016() -> None:
    payload = build_auth_payload_016("seed")
    assert payload["identifier"].startswith("seed-")
