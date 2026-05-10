"""Generated filler test 021 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_021 import build_analytics_payload_021


def test_generated_payload_021() -> None:
    payload = build_analytics_payload_021("seed")
    assert payload["identifier"].startswith("seed-")
