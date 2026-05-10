"""Generated filler test 007 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_007 import build_notifications_payload_007


def test_generated_payload_007() -> None:
    payload = build_notifications_payload_007("seed")
    assert payload["identifier"].startswith("seed-")
