"""Generated filler test 008 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_008 import build_notifications_payload_008


def test_generated_payload_008() -> None:
    payload = build_notifications_payload_008("seed")
    assert payload["identifier"].startswith("seed-")
