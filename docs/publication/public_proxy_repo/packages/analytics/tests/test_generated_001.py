"""Generated filler test 001 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_001 import build_analytics_payload_001


def test_generated_payload_001() -> None:
    payload = build_analytics_payload_001("seed")
    assert payload["identifier"].startswith("seed-")
