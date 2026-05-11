"""Generated filler test 038 for the auth package."""

from __future__ import annotations

from auth.generated.generated_038 import build_auth_payload_038


def test_generated_payload_038() -> None:
    payload = build_auth_payload_038("seed")
    assert payload["identifier"].startswith("seed-")
