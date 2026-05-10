"""Generated filler test 016 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_016 import build_analytics_payload_016


def test_generated_payload_016() -> None:
    payload = build_analytics_payload_016("seed")
    assert payload["identifier"].startswith("seed-")
