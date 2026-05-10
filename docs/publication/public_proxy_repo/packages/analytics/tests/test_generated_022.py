"""Generated filler test 022 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_022 import build_analytics_payload_022


def test_generated_payload_022() -> None:
    payload = build_analytics_payload_022("seed")
    assert payload["identifier"].startswith("seed-")
