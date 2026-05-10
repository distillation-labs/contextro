"""Generated filler test 021 for the shared package."""

from __future__ import annotations

from shared.generated.generated_021 import build_shared_payload_021


def test_generated_payload_021() -> None:
    payload = build_shared_payload_021("seed")
    assert payload["identifier"].startswith("seed-")
