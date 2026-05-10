"""Generated filler test 032 for the auth package."""

from __future__ import annotations

from auth.generated.generated_032 import build_auth_payload_032


def test_generated_payload_032() -> None:
    payload = build_auth_payload_032("seed")
    assert payload["identifier"].startswith("seed-")
