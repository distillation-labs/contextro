"""Generated filler test 030 for the notifications package."""

from __future__ import annotations

from notifications.generated.generated_030 import build_notifications_payload_030


def test_generated_payload_030() -> None:
    payload = build_notifications_payload_030("seed")
    assert payload["identifier"].startswith("seed-")
