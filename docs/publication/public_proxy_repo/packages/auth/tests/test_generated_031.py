"""Generated filler test 031 for the auth package."""

from __future__ import annotations

from auth.generated.generated_031 import build_auth_payload_031


def test_generated_payload_031() -> None:
    payload = build_auth_payload_031("seed")
    assert payload["identifier"].startswith("seed-")
