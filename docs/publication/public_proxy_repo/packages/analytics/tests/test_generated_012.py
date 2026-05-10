"""Generated filler test 012 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_012 import build_analytics_payload_012


def test_generated_payload_012() -> None:
    payload = build_analytics_payload_012("seed")
    assert payload["identifier"].startswith("seed-")
