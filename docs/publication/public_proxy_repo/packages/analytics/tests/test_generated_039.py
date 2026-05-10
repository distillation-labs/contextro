"""Generated filler test 039 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_039 import build_analytics_payload_039


def test_generated_payload_039() -> None:
    payload = build_analytics_payload_039("seed")
    assert payload["identifier"].startswith("seed-")
