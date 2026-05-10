"""Generated filler test 043 for the auth package."""

from __future__ import annotations

from auth.generated.generated_043 import build_auth_payload_043


def test_generated_payload_043() -> None:
    payload = build_auth_payload_043("seed")
    assert payload["identifier"].startswith("seed-")
