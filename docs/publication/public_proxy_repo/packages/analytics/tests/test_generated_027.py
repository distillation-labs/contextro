"""Generated filler test 027 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_027 import build_analytics_payload_027


def test_generated_payload_027() -> None:
    payload = build_analytics_payload_027("seed")
    assert payload["identifier"].startswith("seed-")
