"""Generated filler test 033 for the shared package."""

from __future__ import annotations

from shared.generated.generated_033 import build_shared_payload_033


def test_generated_payload_033() -> None:
    payload = build_shared_payload_033("seed")
    assert payload["identifier"].startswith("seed-")
