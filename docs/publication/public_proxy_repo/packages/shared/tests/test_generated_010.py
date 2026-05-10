"""Generated filler test 010 for the shared package."""

from __future__ import annotations

from shared.generated.generated_010 import build_shared_payload_010


def test_generated_payload_010() -> None:
    payload = build_shared_payload_010("seed")
    assert payload["identifier"].startswith("seed-")
