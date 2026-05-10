"""Generated filler test 026 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_026 import build_analytics_payload_026


def test_generated_payload_026() -> None:
    payload = build_analytics_payload_026("seed")
    assert payload["identifier"].startswith("seed-")
