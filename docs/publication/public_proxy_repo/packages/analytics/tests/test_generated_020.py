"""Generated filler test 020 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_020 import build_analytics_payload_020


def test_generated_payload_020() -> None:
    payload = build_analytics_payload_020("seed")
    assert payload["identifier"].startswith("seed-")
