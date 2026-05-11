"""Generated filler test 009 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_009 import build_analytics_payload_009


def test_generated_payload_009() -> None:
    payload = build_analytics_payload_009("seed")
    assert payload["identifier"].startswith("seed-")
