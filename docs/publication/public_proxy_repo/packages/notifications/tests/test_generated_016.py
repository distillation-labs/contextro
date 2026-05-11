"""Generated filler test 016 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_016 import build_notifications_payload_016


def test_generated_payload_016() -> None:
    payload = build_notifications_payload_016("seed")
    assert payload["identifier"].startswith("seed-")
