"""Generated filler test 036 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_036 import build_analytics_payload_036


def test_generated_payload_036() -> None:
    payload = build_analytics_payload_036("seed")
    assert payload["identifier"].startswith("seed-")
