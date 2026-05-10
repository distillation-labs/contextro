"""Generated filler test 041 for the shared package."""

from __future__ import annotations

from shared.generated.generated_041 import build_shared_payload_041


def test_generated_payload_041() -> None:
    payload = build_shared_payload_041("seed")
    assert payload["identifier"].startswith("seed-")
