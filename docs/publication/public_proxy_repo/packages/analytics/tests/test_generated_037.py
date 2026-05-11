"""Generated filler test 037 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_037 import build_analytics_payload_037


def test_generated_payload_037() -> None:
    payload = build_analytics_payload_037("seed")
    assert payload["identifier"].startswith("seed-")
