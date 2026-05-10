"""Generated filler test 033 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_033 import build_analytics_payload_033


def test_generated_payload_033() -> None:
    payload = build_analytics_payload_033("seed")
    assert payload["identifier"].startswith("seed-")
