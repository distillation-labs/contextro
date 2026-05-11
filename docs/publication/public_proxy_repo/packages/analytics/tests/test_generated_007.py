"""Generated filler test 007 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_007 import build_analytics_payload_007


def test_generated_payload_007() -> None:
    payload = build_analytics_payload_007("seed")
    assert payload["identifier"].startswith("seed-")
