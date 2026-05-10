"""Generated filler test 017 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_017 import build_analytics_payload_017


def test_generated_payload_017() -> None:
    payload = build_analytics_payload_017("seed")
    assert payload["identifier"].startswith("seed-")
