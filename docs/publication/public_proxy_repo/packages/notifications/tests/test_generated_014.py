"""Generated filler test 014 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_014 import build_notifications_payload_014


def test_generated_payload_014() -> None:
    payload = build_notifications_payload_014("seed")
    assert payload["identifier"].startswith("seed-")
