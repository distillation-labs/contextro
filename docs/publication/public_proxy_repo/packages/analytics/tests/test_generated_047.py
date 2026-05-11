"""Generated filler test 047 for the analytics package."""

from __future__ import annotations

from analytics.generated.generated_047 import build_analytics_payload_047


def test_generated_payload_047() -> None:
    payload = build_analytics_payload_047("seed")
    assert payload["identifier"].startswith("seed-")
