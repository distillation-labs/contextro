"""Generated filler test 021 for the auth package."""

from __future__ import annotations

from auth.generated.generated_021 import build_auth_payload_021


def test_generated_payload_021() -> None:
    payload = build_auth_payload_021("seed")
    assert payload["identifier"].startswith("seed-")
