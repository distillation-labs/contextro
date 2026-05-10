"""Generated filler test 008 for the shared package."""

from __future__ import annotations

from shared.generated.generated_008 import build_shared_payload_008


def test_generated_payload_008() -> None:
    payload = build_shared_payload_008("seed")
    assert payload["identifier"].startswith("seed-")
