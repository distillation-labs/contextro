"""Generated filler test 048 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_048 import build_analytics_payload_048


def test_generated_payload_048() -> None:
    payload = build_analytics_payload_048("seed")
    assert payload["identifier"].startswith("seed-")
