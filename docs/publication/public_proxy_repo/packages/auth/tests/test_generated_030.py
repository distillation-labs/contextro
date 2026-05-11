"""Generated filler test 030 for the auth package."""

from __future__ import annotations

from auth.generated.generated_030 import build_auth_payload_030


def test_generated_payload_030() -> None:
    payload = build_auth_payload_030("seed")
    assert payload["identifier"].startswith("seed-")
