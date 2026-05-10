"""Generated filler test 024 for the auth package."""

from __future__ import annotations

from auth.generated.generated_024 import build_auth_payload_024


def test_generated_payload_024() -> None:
    payload = build_auth_payload_024("seed")
    assert payload["identifier"].startswith("seed-")
