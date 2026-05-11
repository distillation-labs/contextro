"""Generated filler test 020 for the auth package."""

from __future__ import annotations

from auth.generated.generated_020 import build_auth_payload_020


def test_generated_payload_020() -> None:
    payload = build_auth_payload_020("seed")
    assert payload["identifier"].startswith("seed-")
