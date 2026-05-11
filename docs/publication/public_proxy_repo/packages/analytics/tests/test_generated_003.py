"""Generated filler test 003 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_003 import build_analytics_payload_003


def test_generated_payload_003() -> None:
    payload = build_analytics_payload_003("seed")
    assert payload["identifier"].startswith("seed-")
