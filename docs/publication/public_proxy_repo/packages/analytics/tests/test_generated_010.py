"""Generated filler test 010 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_010 import build_analytics_payload_010


def test_generated_payload_010() -> None:
    payload = build_analytics_payload_010("seed")
    assert payload["identifier"].startswith("seed-")
