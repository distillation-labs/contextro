"""Generated filler test 034 for the auth package."""

from __future__ import annotations

from auth.generated.generated_034 import build_auth_payload_034


def test_generated_payload_034() -> None:
    payload = build_auth_payload_034("seed")
    assert payload["identifier"].startswith("seed-")
