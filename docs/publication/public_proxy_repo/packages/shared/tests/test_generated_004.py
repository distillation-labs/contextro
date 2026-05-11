"""Generated filler test 004 for the shared package."""

from __future__ import annotations

from shared.generated.generated_004 import build_shared_payload_004


def test_generated_payload_004() -> None:
    payload = build_shared_payload_004("seed")
    assert payload["identifier"].startswith("seed-")
