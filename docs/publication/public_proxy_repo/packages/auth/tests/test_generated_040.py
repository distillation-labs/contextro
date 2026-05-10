"""Generated filler test 040 for the auth package."""

from __future__ import annotations

from auth.generated.generated_040 import build_auth_payload_040


def test_generated_payload_040() -> None:
    payload = build_auth_payload_040("seed")
    assert payload["identifier"].startswith("seed-")
