"""Generated filler test 032 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_032 import build_analytics_payload_032


def test_generated_payload_032() -> None:
    payload = build_analytics_payload_032("seed")
    assert payload["identifier"].startswith("seed-")
