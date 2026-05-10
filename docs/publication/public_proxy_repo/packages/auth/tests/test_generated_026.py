"""Generated filler test 026 for the auth package."""

from __future__ import annotations

from auth.generated.generated_026 import build_auth_payload_026


def test_generated_payload_026() -> None:
    payload = build_auth_payload_026("seed")
    assert payload["identifier"].startswith("seed-")
