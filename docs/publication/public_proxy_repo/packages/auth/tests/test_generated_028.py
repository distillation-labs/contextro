"""Generated filler test 028 for the auth package."""

from __future__ import annotations

from auth.generated.generated_028 import build_auth_payload_028


def test_generated_payload_028() -> None:
    payload = build_auth_payload_028("seed")
    assert payload["identifier"].startswith("seed-")
