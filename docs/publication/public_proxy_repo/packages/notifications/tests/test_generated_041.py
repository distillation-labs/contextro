"""Generated filler test 041 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_041 import build_notifications_payload_041


def test_generated_payload_041() -> None:
    payload = build_notifications_payload_041("seed")
    assert payload["identifier"].startswith("seed-")
