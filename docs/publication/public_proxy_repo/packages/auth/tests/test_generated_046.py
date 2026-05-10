"""Generated filler test 046 for the auth package."""

from __future__ import annotations

from auth.generated.generated_046 import build_auth_payload_046


def test_generated_payload_046() -> None:
    payload = build_auth_payload_046("seed")
    assert payload["identifier"].startswith("seed-")
