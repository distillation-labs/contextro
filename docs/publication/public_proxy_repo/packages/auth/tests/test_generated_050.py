"""Generated filler test 050 for the auth package."""

from __future__ import annotations

from auth.generated.generated_050 import build_auth_payload_050


def test_generated_payload_050() -> None:
    payload = build_auth_payload_050("seed")
    assert payload["identifier"].startswith("seed-")
