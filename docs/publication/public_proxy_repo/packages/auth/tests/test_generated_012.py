"""Generated filler test 012 for the auth package."""

from __future__ import annotations

from auth.generated.generated_012 import build_auth_payload_012


def test_generated_payload_012() -> None:
    payload = build_auth_payload_012("seed")
    assert payload["identifier"].startswith("seed-")
