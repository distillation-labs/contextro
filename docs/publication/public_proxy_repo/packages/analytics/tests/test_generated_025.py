"""Generated filler test 025 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_025 import build_analytics_payload_025


def test_generated_payload_025() -> None:
    payload = build_analytics_payload_025("seed")
    assert payload["identifier"].startswith("seed-")
