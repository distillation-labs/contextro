"""Generated filler test 005 for the auth package."""

from __future__ import annotations

from auth.generated.generated_005 import build_auth_payload_005


def test_generated_payload_005() -> None:
    payload = build_auth_payload_005("seed")
    assert payload["identifier"].startswith("seed-")
