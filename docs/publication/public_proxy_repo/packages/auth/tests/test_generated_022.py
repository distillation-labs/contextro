"""Generated filler test 022 for the auth package."""

from __future__ import annotations

from auth.generated.generated_022 import build_auth_payload_022


def test_generated_payload_022() -> None:
    payload = build_auth_payload_022("seed")
    assert payload["identifier"].startswith("seed-")
