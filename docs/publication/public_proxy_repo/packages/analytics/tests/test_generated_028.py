"""Generated filler test 028 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_028 import build_analytics_payload_028


def test_generated_payload_028() -> None:
    payload = build_analytics_payload_028("seed")
    assert payload["identifier"].startswith("seed-")
