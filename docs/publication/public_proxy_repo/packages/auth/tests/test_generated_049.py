"""Generated filler test 049 for the auth package."""

from __future__ import annotations

from auth.generated.generated_049 import build_auth_payload_049


def test_generated_payload_049() -> None:
    payload = build_auth_payload_049("seed")
    assert payload["identifier"].startswith("seed-")
