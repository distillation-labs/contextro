"""Generated filler test 007 for the shared package."""

from __future__ import annotations

from shared.generated.generated_007 import build_shared_payload_007


def test_generated_payload_007() -> None:
    payload = build_shared_payload_007("seed")
    assert payload["identifier"].startswith("seed-")
