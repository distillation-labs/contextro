"""Generated filler test 023 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_023 import build_analytics_payload_023


def test_generated_payload_023() -> None:
    payload = build_analytics_payload_023("seed")
    assert payload["identifier"].startswith("seed-")
