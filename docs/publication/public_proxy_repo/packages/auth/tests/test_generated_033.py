"""Generated filler test 033 for the auth package."""

from __future__ import annotations

from auth.generated.generated_033 import build_auth_payload_033


def test_generated_payload_033() -> None:
    payload = build_auth_payload_033("seed")
    assert payload["identifier"].startswith("seed-")
