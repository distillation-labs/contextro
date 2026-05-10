"""Generated filler test 038 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_038 import build_analytics_payload_038


def test_generated_payload_038() -> None:
    payload = build_analytics_payload_038("seed")
    assert payload["identifier"].startswith("seed-")
