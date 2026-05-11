"""Generated filler test 039 for the auth package."""

from __future__ import annotations

from auth.generated.generated_039 import build_auth_payload_039


def test_generated_payload_039() -> None:
    payload = build_auth_payload_039("seed")
    assert payload["identifier"].startswith("seed-")
