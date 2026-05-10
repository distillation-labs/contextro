"""Generated filler test 037 for the auth package."""

from __future__ import annotations

from auth.generated.generated_037 import build_auth_payload_037


def test_generated_payload_037() -> None:
    payload = build_auth_payload_037("seed")
    assert payload["identifier"].startswith("seed-")
