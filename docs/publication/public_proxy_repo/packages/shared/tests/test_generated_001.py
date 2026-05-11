"""Generated filler test 001 for the shared package."""

from __future__ import annotations

from shared.generated.generated_001 import build_shared_payload_001


def test_generated_payload_001() -> None:
    payload = build_shared_payload_001("seed")
    assert payload["identifier"].startswith("seed-")
