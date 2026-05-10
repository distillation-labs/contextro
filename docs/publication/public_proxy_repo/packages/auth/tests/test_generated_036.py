"""Generated filler test 036 for the auth package."""

from __future__ import annotations

from auth.generated.generated_036 import build_auth_payload_036


def test_generated_payload_036() -> None:
    payload = build_auth_payload_036("seed")
    assert payload["identifier"].startswith("seed-")
