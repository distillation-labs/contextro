"""Generated filler test 040 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_040 import build_analytics_payload_040


def test_generated_payload_040() -> None:
    payload = build_analytics_payload_040("seed")
    assert payload["identifier"].startswith("seed-")
