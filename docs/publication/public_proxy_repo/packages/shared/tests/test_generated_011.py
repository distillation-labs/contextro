"""Generated filler test 011 for the shared package."""

from __future__ import annotations

from shared.generated.generated_011 import build_shared_payload_011


def test_generated_payload_011() -> None:
    payload = build_shared_payload_011("seed")
    assert payload["identifier"].startswith("seed-")
