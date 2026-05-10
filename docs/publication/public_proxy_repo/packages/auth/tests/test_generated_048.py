"""Generated filler test 048 for the auth package."""

from __future__ import annotations

from auth.generated.generated_048 import build_auth_payload_048


def test_generated_payload_048() -> None:
    payload = build_auth_payload_048("seed")
    assert payload["identifier"].startswith("seed-")
