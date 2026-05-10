"""Generated filler test 041 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_041 import build_analytics_payload_041


def test_generated_payload_041() -> None:
    payload = build_analytics_payload_041("seed")
    assert payload["identifier"].startswith("seed-")
