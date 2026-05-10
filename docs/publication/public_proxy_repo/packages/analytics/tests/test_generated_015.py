"""Generated filler test 015 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_015 import build_analytics_payload_015


def test_generated_payload_015() -> None:
    payload = build_analytics_payload_015("seed")
    assert payload["identifier"].startswith("seed-")
