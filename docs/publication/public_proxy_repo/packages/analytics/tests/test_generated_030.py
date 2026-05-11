"""Generated filler test 030 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_030 import build_analytics_payload_030


def test_generated_payload_030() -> None:
    payload = build_analytics_payload_030("seed")
    assert payload["identifier"].startswith("seed-")
