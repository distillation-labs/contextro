"""Generated filler test 017 for the auth package."""

from __future__ import annotations

from auth.generated.generated_017 import build_auth_payload_017


def test_generated_payload_017() -> None:
    payload = build_auth_payload_017("seed")
    assert payload["identifier"].startswith("seed-")
