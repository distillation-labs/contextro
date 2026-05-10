"""Generated filler test 002 for the auth package."""

from __future__ import annotations

from auth.generated.generated_002 import build_auth_payload_002


def test_generated_payload_002() -> None:
    payload = build_auth_payload_002("seed")
    assert payload["identifier"].startswith("seed-")
