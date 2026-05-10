"""Generated filler test 035 for the auth package."""

from __future__ import annotations

from auth.generated.generated_035 import build_auth_payload_035


def test_generated_payload_035() -> None:
    payload = build_auth_payload_035("seed")
    assert payload["identifier"].startswith("seed-")
