"""Generated filler test 031 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_031 import build_analytics_payload_031


def test_generated_payload_031() -> None:
    payload = build_analytics_payload_031("seed")
    assert payload["identifier"].startswith("seed-")
