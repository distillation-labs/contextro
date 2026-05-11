"""Generated filler test 013 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_013 import build_analytics_payload_013


def test_generated_payload_013() -> None:
    payload = build_analytics_payload_013("seed")
    assert payload["identifier"].startswith("seed-")
