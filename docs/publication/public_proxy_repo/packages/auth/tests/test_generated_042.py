"""Generated filler test 042 for the auth package."""

from __future__ import annotations

from auth.generated.generated_042 import build_auth_payload_042


def test_generated_payload_042() -> None:
    payload = build_auth_payload_042("seed")
    assert payload["identifier"].startswith("seed-")
