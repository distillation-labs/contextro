"""Generated filler test 043 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_043 import build_analytics_payload_043


def test_generated_payload_043() -> None:
    payload = build_analytics_payload_043("seed")
    assert payload["identifier"].startswith("seed-")
