"""Generated filler test 047 for the auth package."""

from __future__ import annotations

from auth.generated.generated_047 import build_auth_payload_047


def test_generated_payload_047() -> None:
    payload = build_auth_payload_047("seed")
    assert payload["identifier"].startswith("seed-")
