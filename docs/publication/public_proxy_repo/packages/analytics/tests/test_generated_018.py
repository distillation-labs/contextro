"""Generated filler test 018 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_018 import build_analytics_payload_018


def test_generated_payload_018() -> None:
    payload = build_analytics_payload_018("seed")
    assert payload["identifier"].startswith("seed-")
