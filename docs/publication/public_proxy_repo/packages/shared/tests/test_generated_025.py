"""Generated filler test 025 for the shared package."""

from __future__ import annotations

from shared.generated.generated_025 import build_shared_payload_025


def test_generated_payload_025() -> None:
    payload = build_shared_payload_025("seed")
    assert payload["identifier"].startswith("seed-")
