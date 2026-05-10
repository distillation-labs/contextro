"""Generated filler test 018 for the shared package."""

from __future__ import annotations

from shared.generated.generated_018 import build_shared_payload_018


def test_generated_payload_018() -> None:
    payload = build_shared_payload_018("seed")
    assert payload["identifier"].startswith("seed-")
