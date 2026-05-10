"""Generated filler test 025 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_025 import build_notifications_payload_025


def test_generated_payload_025() -> None:
    payload = build_notifications_payload_025("seed")
    assert payload["identifier"].startswith("seed-")
