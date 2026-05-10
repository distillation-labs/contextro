"""Generated filler test 009 for the shared package."""

from __future__ import annotations

from shared.generated.generated_009 import build_shared_payload_009


def test_generated_payload_009() -> None:
    payload = build_shared_payload_009("seed")
    assert payload["identifier"].startswith("seed-")
