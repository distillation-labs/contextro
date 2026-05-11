"""Generated filler test 049 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_049 import build_analytics_payload_049


def test_generated_payload_049() -> None:
    payload = build_analytics_payload_049("seed")
    assert payload["identifier"].startswith("seed-")
