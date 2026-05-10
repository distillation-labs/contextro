"""Generated filler test 019 for the shared package."""

from __future__ import annotations

from shared.generated.generated_019 import build_shared_payload_019


def test_generated_payload_019() -> None:
    payload = build_shared_payload_019("seed")
    assert payload["identifier"].startswith("seed-")
