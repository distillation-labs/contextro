"""Generated filler test 034 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_034 import build_analytics_payload_034


def test_generated_payload_034() -> None:
    payload = build_analytics_payload_034("seed")
    assert payload["identifier"].startswith("seed-")
