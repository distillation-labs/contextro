"""Generated filler test 013 for the auth package."""

from __future__ import annotations

from auth.generated.generated_013 import build_auth_payload_013


def test_generated_payload_013() -> None:
    payload = build_auth_payload_013("seed")
    assert payload["identifier"].startswith("seed-")
