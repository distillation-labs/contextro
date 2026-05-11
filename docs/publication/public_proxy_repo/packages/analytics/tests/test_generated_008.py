"""Generated filler test 008 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_008 import build_analytics_payload_008


def test_generated_payload_008() -> None:
    payload = build_analytics_payload_008("seed")
    assert payload["identifier"].startswith("seed-")
