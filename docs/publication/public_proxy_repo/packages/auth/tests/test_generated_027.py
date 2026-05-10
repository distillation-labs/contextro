"""Generated filler test 027 for the auth package."""

from __future__ import annotations

from auth.generated.generated_027 import build_auth_payload_027


def test_generated_payload_027() -> None:
    payload = build_auth_payload_027("seed")
    assert payload["identifier"].startswith("seed-")
