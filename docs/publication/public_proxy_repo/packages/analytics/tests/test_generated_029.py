"""Generated filler test 029 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_029 import build_analytics_payload_029


def test_generated_payload_029() -> None:
    payload = build_analytics_payload_029("seed")
    assert payload["identifier"].startswith("seed-")
