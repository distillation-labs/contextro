"""Generated filler test 044 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_044 import build_analytics_payload_044


def test_generated_payload_044() -> None:
    payload = build_analytics_payload_044("seed")
    assert payload["identifier"].startswith("seed-")
