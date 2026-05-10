"""Generated filler test 044 for the shared package."""

from __future__ import annotations

from shared.generated.generated_044 import build_shared_payload_044


def test_generated_payload_044() -> None:
    payload = build_shared_payload_044("seed")
    assert payload["identifier"].startswith("seed-")
