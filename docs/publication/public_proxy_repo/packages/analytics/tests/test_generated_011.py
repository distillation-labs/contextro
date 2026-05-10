"""Generated filler test 011 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_011 import build_analytics_payload_011


def test_generated_payload_011() -> None:
    payload = build_analytics_payload_011("seed")
    assert payload["identifier"].startswith("seed-")
