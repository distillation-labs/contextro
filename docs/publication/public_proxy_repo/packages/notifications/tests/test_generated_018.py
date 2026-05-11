"""Generated filler test 018 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_018 import build_notifications_payload_018


def test_generated_payload_018() -> None:
    payload = build_notifications_payload_018("seed")
    assert payload["identifier"].startswith("seed-")
