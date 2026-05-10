"""Generated filler test 029 for the auth package."""

from __future__ import annotations

from auth.generated.generated_029 import build_auth_payload_029


def test_generated_payload_029() -> None:
    payload = build_auth_payload_029("seed")
    assert payload["identifier"].startswith("seed-")
