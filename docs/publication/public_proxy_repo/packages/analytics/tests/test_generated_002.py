"""Generated filler test 002 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_002 import build_analytics_payload_002


def test_generated_payload_002() -> None:
    payload = build_analytics_payload_002("seed")
    assert payload["identifier"].startswith("seed-")
