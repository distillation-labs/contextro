"""Generated filler test 003 for the auth package."""

from __future__ import annotations

from auth.generated.generated_003 import build_auth_payload_003


def test_generated_payload_003() -> None:
    payload = build_auth_payload_003("seed")
    assert payload["identifier"].startswith("seed-")
