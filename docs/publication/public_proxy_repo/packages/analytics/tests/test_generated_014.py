"""Generated filler test 014 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_014 import build_analytics_payload_014


def test_generated_payload_014() -> None:
    payload = build_analytics_payload_014("seed")
    assert payload["identifier"].startswith("seed-")
