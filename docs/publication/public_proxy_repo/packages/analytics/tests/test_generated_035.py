"""Generated filler test 035 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_035 import build_analytics_payload_035


def test_generated_payload_035() -> None:
    payload = build_analytics_payload_035("seed")
    assert payload["identifier"].startswith("seed-")
