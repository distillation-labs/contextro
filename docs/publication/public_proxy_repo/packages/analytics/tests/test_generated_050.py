"""Generated filler test 050 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_050 import build_analytics_payload_050


def test_generated_payload_050() -> None:
    payload = build_analytics_payload_050("seed")
    assert payload["identifier"].startswith("seed-")
