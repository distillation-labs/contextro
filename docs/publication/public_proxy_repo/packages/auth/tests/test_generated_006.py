"""Generated filler test 006 for the auth package."""

from __future__ import annotations

from auth.generated.generated_006 import build_auth_payload_006


def test_generated_payload_006() -> None:
    payload = build_auth_payload_006("seed")
    assert payload["identifier"].startswith("seed-")
