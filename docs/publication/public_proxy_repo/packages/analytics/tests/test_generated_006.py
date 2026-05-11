"""Generated filler test 006 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_006 import build_analytics_payload_006


def test_generated_payload_006() -> None:
    payload = build_analytics_payload_006("seed")
    assert payload["identifier"].startswith("seed-")
