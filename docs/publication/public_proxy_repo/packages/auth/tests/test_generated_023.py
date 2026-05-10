"""Generated filler test 023 for the auth package."""

from __future__ import annotations

from auth.generated.generated_023 import build_auth_payload_023


def test_generated_payload_023() -> None:
    payload = build_auth_payload_023("seed")
    assert payload["identifier"].startswith("seed-")
